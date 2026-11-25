//! Resolve references before applying explicit page and resource scope.
use crate::{ProjectConfig, ResourceKind, SkipReason};
use url::Url;

#[derive(Debug, thiserror::Error)]
#[error("Only HTTP(S) URLs without embedded credentials are allowed")]
pub struct PolicyError;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Skip(SkipReason),
}

/// HTML remains available for navigation; allowlists select published assets.
pub fn save_allowed(config: &ProjectConfig, url: &Url, kind: ResourceKind, mime: &str) -> bool {
    if kind == ResourceKind::Page {
        return true;
    }
    let mime = crate::media_type(mime);
    let extension = url
        .path()
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    (config.mime_types.is_empty()
        || config
            .mime_types
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(&mime)))
        && (config.extensions.is_empty()
            || extension.is_some_and(|ext| config.extensions.contains(&ext)))
}

pub fn normalize_url(base: &Url, reference: &str) -> Result<Url, PolicyError> {
    let mut u = base.join(reference).map_err(|_| PolicyError)?;
    if u.as_str().len() > 8192
        || !matches!(u.scheme(), "http" | "https")
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err(PolicyError);
    }
    u.set_fragment(None);
    Ok(u)
}

pub fn evaluate(c: &ProjectConfig, u: &Url, kind: ResourceKind, depth: u16) -> Decision {
    if normalize_url(u, u.as_str()).is_err() {
        return Decision::Skip(SkipReason::Scheme);
    }
    let origins = if kind == ResourceKind::Page {
        &c.page_origins
    } else {
        &c.resource_origins
    };
    if !origins.contains(&u.origin().ascii_serialization())
        || (kind == ResourceKind::Page && !u.path().starts_with(&c.path_prefix) && *u != c.seed)
    {
        return Decision::Skip(SkipReason::Scope);
    }
    if kind == ResourceKind::Page && depth > c.limits.depth {
        return Decision::Skip(SkipReason::Depth);
    }
    if c.exclude_prefixes.iter().any(|p| u.as_str().starts_with(p))
        || (!c.include_prefixes.is_empty()
            && !c.include_prefixes.iter().any(|p| u.as_str().starts_with(p)))
    {
        return Decision::Skip(SkipReason::Filter);
    }
    Decision::Allow
}
