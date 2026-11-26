//! Structural, size-capped HTML and CSS discovery. No script execution.
use crate::{ResourceKind, UrlKey, normalize_url};
use cssparser::{Parser, Token};
use lol_html::{HtmlRewriter, MemorySettings, Settings, element, text};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
};
use url::Url;

pub const PARSE_CAP: usize = 8 << 20;
#[derive(Debug, thiserror::Error)]
#[error("The document is too large, unsupported, or malformed")]
pub struct ParseError;
#[derive(Clone, Debug)]
pub struct DiscoveredLink {
    pub url: Url,
    pub kind: ResourceKind,
    pub fragment: Option<String>,
}

pub(crate) fn settings<'a>() -> Settings<'a, 'a> {
    Settings::new()
        .with_memory_settings(MemorySettings::new().with_max_allowed_memory_usage(PARSE_CAP))
}

pub(crate) fn html_base(body: &[u8], base: &Url) -> Result<Url, ParseError> {
    let mut found = None;
    let mut parser = HtmlRewriter::new(
        settings().append_element_content_handler(element!("base[href]", |el| {
            if found.is_none() {
                found = el
                    .get_attribute("href")
                    .and_then(|s| normalize_url(base, &s).ok());
            }
            Ok(())
        })),
        |_: &[u8]| {},
    );
    parser.write(body).map_err(|_| ParseError)?;
    parser.end().map_err(|_| ParseError)?;
    Ok(found.unwrap_or_else(|| base.clone()))
}

/// Source spans allow rewriting escaped CSS URLs without altering other tokens.
pub(crate) fn css_urls(css: &str) -> Vec<(std::ops::Range<usize>, String)> {
    fn scan(p: &mut Parser<'_>, out: &mut Vec<(std::ops::Range<usize>, String)>, depth: usize) {
        if depth > 64 {
            return;
        }
        let mut import = false;
        loop {
            let start = p.position().byte_index();
            let Ok(t) = p.next_including_whitespace_and_comments().cloned() else {
                break;
            };
            match t {
                Token::AtKeyword(v) if v.eq_ignore_ascii_case("import") => import = true,
                Token::UnquotedUrl(v) => {
                    out.push((start..p.position().byte_index(), v.to_string()));
                    import = false;
                }
                Token::QuotedString(v) if import => {
                    out.push((start..p.position().byte_index(), v.to_string()));
                    import = false;
                }
                Token::Function(v) if v.eq_ignore_ascii_case("url") => {
                    let mut value = None;
                    let _: Result<(), cssparser::ParseError<()>> = p.parse_nested_block(|nested| {
                        if let Ok(Token::QuotedString(s)) = nested.next() {
                            value = Some(s.to_string());
                        }
                        while nested.next_including_whitespace_and_comments().is_ok() {}
                        Ok(())
                    });
                    if let Some(v) = value {
                        out.push((start..p.position().byte_index(), v));
                    }
                    import = false;
                }
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    let _: Result<(), cssparser::ParseError<()>> = p.parse_nested_block(|nested| {
                        scan(nested, out, depth + 1);
                        Ok(())
                    });
                    import = false;
                }
                Token::WhiteSpace(_) | Token::Comment(_) => {}
                _ => import = false,
            }
        }
    }
    let mut out = Vec::new();
    scan(&mut Parser::new(css), &mut out, 0);
    out
}

/// HTML srcset candidates: a URL followed by optional density/width descriptors.
/// Data URLs consume internal commas until whitespace, as required by HTML.
pub(crate) fn srcset(value: &str) -> Vec<(String, String)> {
    let mut rest = value.trim_start();
    let mut result = Vec::new();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            break;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let raw = &rest[..end];
        let url = raw.trim_end_matches(',');
        rest = &rest[end..];
        let descriptor = if raw.ends_with(',') {
            ""
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            let d = rest[..end].trim();
            rest = &rest[end..];
            d
        };
        if !url.is_empty() {
            result.push((url.into(), descriptor.into()));
        }
    }
    result
}

pub(crate) fn attributes(tag: &str, rel: &str) -> Vec<(&'static str, ResourceKind)> {
    use ResourceKind::{Asset, Page};
    match tag {
        "a" | "area" => vec![("href", Page)],
        "iframe" | "frame" => vec![("src", Page)],
        "link"
            if rel
                .split_whitespace()
                .any(|r| matches!(r, "stylesheet" | "icon" | "preload")) =>
        {
            vec![("href", Asset)]
        }
        "img" | "source" | "audio" | "script" | "track" | "input" => vec![("src", Asset)],
        "video" => vec![("src", Asset), ("poster", Asset)],
        _ => Vec::new(),
    }
}

pub fn discover(
    body: &[u8],
    mime: &str,
    final_url: &Url,
) -> Result<Vec<DiscoveredLink>, ParseError> {
    let (links, truncated) = discover_bounded(body, mime, final_url, 100_000)?;
    if truncated {
        Err(ParseError)
    } else {
        Ok(links)
    }
}
pub(crate) fn discover_bounded(
    body: &[u8],
    mime: &str,
    final_url: &Url,
    limit: usize,
) -> Result<(Vec<DiscoveredLink>, bool), ParseError> {
    let mime = crate::media_type(mime);
    if !matches!(
        mime.as_str(),
        "text/css" | "text/html" | "application/xhtml+xml"
    ) {
        return Ok((Vec::new(), false));
    }
    if body.len() > PARSE_CAP {
        return Err(ParseError);
    }
    let links = RefCell::new(Vec::new());
    let known = RefCell::new(BTreeSet::new());
    let truncated = Cell::new(false);
    let add = |reference: &str, kind, base: &Url| {
        if reference.starts_with('#') {
            return;
        }
        if let Ok(raw) = base.join(reference)
            && let Ok(url) = normalize_url(base, reference)
        {
            let key = UrlKey::of(&url);
            if known.borrow().contains(&key) {
                return;
            }
            if links.borrow().len() >= limit {
                truncated.set(true);
                return;
            }
            known.borrow_mut().insert(key);
            let kind = if kind == ResourceKind::Page
                && crate::classify::link_role(&url) == crate::LinkRole::File
            {
                ResourceKind::Asset
            } else {
                kind
            };
            links.borrow_mut().push(DiscoveredLink {
                url,
                kind,
                fragment: raw.fragment().map(str::to_owned),
            });
        }
    };
    if mime == "text/css" {
        let css = std::str::from_utf8(body).map_err(|_| ParseError)?;
        for (_, url) in css_urls(css) {
            add(&url, ResourceKind::Asset, final_url);
        }
    } else if matches!(mime.as_str(), "text/html" | "application/xhtml+xml") {
        let base = html_base(body, final_url)?;
        let style = RefCell::new(String::new());
        let mut parser = HtmlRewriter::new(
            settings()
                .append_element_content_handler(element!("*", |el| {
                    for (attr, kind) in
                        attributes(&el.tag_name(), &el.get_attribute("rel").unwrap_or_default())
                    {
                        if let Some(v) = el.get_attribute(attr) {
                            add(&v, kind, &base);
                        }
                    }
                    if let Some(v) = el.get_attribute("srcset") {
                        for (url, _) in srcset(&v) {
                            add(&url, ResourceKind::Asset, &base);
                        }
                    }
                    if let Some(v) = el.get_attribute("style") {
                        for (_, url) in css_urls(&v) {
                            add(&url, ResourceKind::Asset, &base);
                        }
                    }
                    Ok(())
                }))
                .append_element_content_handler(text!("style", |t| {
                    style.borrow_mut().push_str(t.as_str());
                    if t.last_in_text_node() {
                        for (_, url) in css_urls(&style.borrow()) {
                            add(&url, ResourceKind::Asset, &base);
                        }
                        style.borrow_mut().clear();
                    }
                    Ok(())
                })),
            |_: &[u8]| {},
        );
        parser.write(body).map_err(|_| ParseError)?;
        parser.end().map_err(|_| ParseError)?;
    }
    Ok((links.into_inner(), truncated.get()))
}
