//! Immutable capture constraints carried into the normal segmented download engine.
use crate::*;
use sdl_engine::{
    Options, Proxy, Route,
    crawl_fetch::{
        BodyBudget, CaptureGuard, CrawlAccess, CrawlLimits, CrawlRequest, NetworkAccess,
        fetch_crawl,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CaptureRoute {
    Direct,
    System,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdapterSnapshot {
    pub label: String,
    pub name: String,
    pub index: u32,
    pub v4: Option<Ipv4Addr>,
    pub v6: Option<Ipv6Addr>,
    pub loopback_too: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapturePolicy {
    pub config: ProjectConfig,
    pub route: CaptureRoute,
    pub adapter: Option<AdapterSnapshot>,
    pub dns: sdl_engine::DnsChoice,
    pub credential: Option<CredentialRef>,
    pub file_rules: catalog::FileRules,
}
impl CapturePolicy {
    pub fn snapshot(project: &catalog::FileProject, options: &Options) -> io::Result<Self> {
        let route = match options.proxy {
            Proxy::None => CaptureRoute::Direct,
            Proxy::System => CaptureRoute::System,
            _ => {
                return Err(io::Error::other(
                    "Site Grabber cannot verify proxy-side DNS for the selected route",
                ));
            }
        };
        let adapter = match &options.route {
            Route::Any => None,
            Route::Down { .. } => {
                return Err(io::Error::other("Selected network adapter is unavailable"));
            }
            Route::Bound(b) => Some(AdapterSnapshot {
                label: b.label.clone(),
                name: b.name.clone(),
                index: b.index,
                v4: b.v4,
                v6: b.v6,
                loopback_too: b.loopback_too,
            }),
        };
        Ok(Self {
            config: project.config.clone(),
            route,
            adapter,
            dns: options.dns.clone(),
            credential: project.credential.clone(),
            file_rules: project.rules.clone(),
        })
    }
    pub fn apply(&self, options: &mut Options) {
        options.proxy = match self.route {
            CaptureRoute::Direct => Proxy::None,
            CaptureRoute::System => Proxy::System,
        };
        options.route = self.adapter.as_ref().map_or(Route::Any, |b| {
            Route::Bound(Arc::new(sdl_engine::net::Binding {
                label: b.label.clone(),
                name: b.name.clone(),
                index: b.index,
                v4: b.v4,
                v6: b.v6,
                loopback_too: b.loopback_too,
            }))
        });
        options.dns = self.dns.clone();
        options.post = None;
        options.unlocker = None;
    }
    pub fn network(&self) -> NetworkAccess {
        let network = if self.config.allow_private_seed {
            NetworkAccess::with_private_origin(self.config.seed.origin().ascii_serialization())
        } else {
            NetworkAccess::public()
        };
        network.restrict(self.config.resource_origins.clone())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteDownloadRequest {
    pub project_id: ProjectId,
    pub candidate_id: UrlKey,
    pub scan_revision: u64,
    pub url: Url,
    pub name: String,
    pub destination: PathBuf,
    pub expected_bytes: Option<u64>,
    pub source_page: Url,
    pub policy: CapturePolicy,
    pub dispatch_id: String,
    pub item_id: u64,
}
impl SiteDownloadRequest {
    pub fn identity(project: &ProjectId, candidate: &UrlKey) -> String {
        format!("{}-{}", project.as_str(), candidate.0)
    }
    pub fn validate(&self) -> io::Result<()> {
        self.policy.config.validate().map_err(io::Error::other)?;
        if self.dispatch_id != Self::identity(&self.project_id, &self.candidate_id)
            || self.item_id == 0
            || self.scan_revision == 0
            || self.name.is_empty()
        {
            return Err(io::Error::other("Invalid guarded dispatch identity"));
        }
        normalize_url(&self.url, self.url.as_str()).map_err(io::Error::other)?;
        if evaluate(&self.policy.config, &self.url, ResourceKind::Asset, 0) != Decision::Allow {
            return Err(io::Error::other("Queued URL is outside capture policy"));
        }
        let root = paths::checked_path(&self.policy.config.destination, std::path::Path::new(""))?;
        let relative = self
            .destination
            .strip_prefix(&root)
            .map_err(|_| io::Error::other("Queued destination is outside capture root"))?;
        paths::checked_path(&root, relative)?;
        if self
            .policy
            .credential
            .as_ref()
            .is_some_and(|c| c.origin != self.policy.config.seed.origin().ascii_serialization())
        {
            return Err(io::Error::other("Invalid queued session scope"));
        }
        Ok(())
    }
    pub fn guard(&self, mut options: Options) -> io::Result<(Options, CaptureGuard)> {
        self.validate()?;
        self.policy.apply(&mut options);
        let guard = CaptureGuard(Arc::new(RuntimeGuard {
            policy: self.policy.clone(),
            options: options.clone(),
        }));
        Ok((options, guard))
    }
}
struct RuntimeGuard {
    policy: CapturePolicy,
    options: Options,
}
impl CrawlAccess for RuntimeGuard {
    fn check_url(&self, url: &Url) -> Result<(), String> {
        self.policy.network().check_url(url)?;
        if evaluate(&self.policy.config, url, ResourceKind::Asset, 0) != Decision::Allow {
            return Err("URL is outside the queued capture policy".into());
        }
        Ok(())
    }
    fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), String> {
        self.policy.network().check_ip(url, ip)
    }
    fn redirect_limit(&self) -> usize {
        usize::from(self.policy.config.limits.redirects)
    }
    fn check_response(&self, url: &Url, mime: &str, size: Option<u64>) -> Result<(), String> {
        if size.is_none()
            && (self.policy.file_rules.minimum_bytes.is_some()
                || self.policy.file_rules.maximum_bytes.is_some())
        {
            return Err(
                "Cannot verify the selected file size limits; server supplied no total length"
                    .into(),
            );
        }
        let metadata = CandidateMetadata {
            url: url.clone(),
            final_url: url.clone(),
            name: url
                .path_segments()
                .and_then(|mut p| p.next_back())
                .unwrap_or("download")
                .into(),
            mime: Some(mime.into()),
            size,
            source_page: self.policy.config.seed.clone(),
        };
        if !self.policy.file_rules.allows(&metadata)
            || !policy::save_allowed(&self.policy.config, url, ResourceKind::Asset, mime)
        {
            return Err("Response no longer matches the queued file filters".into());
        }
        Ok(())
    }
    fn before_request<'a>(
        &'a self,
        url: &'a Url,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            self.check_url(url)?;
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let name = format!(
                "swoopdl-robots-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let directory = std::env::temp_dir().join(name);
            std::fs::create_dir(&directory).map_err(|_| "Robots temporary folder unavailable")?;
            struct Temporary(PathBuf);
            impl Drop for Temporary {
                fn drop(&mut self) {
                    let _ = std::fs::remove_file(self.0.join("body"));
                    let _ = std::fs::remove_dir(&self.0);
                }
            }
            let temporary = Temporary(directory);
            let destination = temporary.0.join("body");
            let origin = url.origin().ascii_serialization();
            let network = if self.policy.config.allow_private_seed {
                NetworkAccess::with_private_origin(
                    self.policy.config.seed.origin().ascii_serialization(),
                )
            } else {
                NetworkAccess::public()
            };
            let response = fetch_crawl(
                CrawlRequest {
                    url: Url::parse(&format!("{origin}/robots.txt"))
                        .map_err(|_| "Invalid robots URL")?,
                    headers: Default::default(),
                    destination: destination.clone(),
                },
                self.options.clone(),
                CrawlLimits {
                    body_bytes: self.policy.config.limits.robots_bytes,
                    ..Default::default()
                },
                Arc::new(BodyBudget::new(self.policy.config.limits.robots_bytes, 0)),
                Arc::new(network.restrict([origin].into())),
                Default::default(),
            )
            .await
            .map_err(|_| "Fresh robots verification failed")?;
            let robots = if response.status.is_success() {
                crate::robots::Robots::parse(
                    &std::fs::read_to_string(destination).map_err(|_| "Robots body unavailable")?,
                )
            } else if response.status.is_client_error()
                && !matches!(response.status.as_u16(), 401 | 403 | 408 | 429)
            {
                crate::robots::Robots::default()
            } else {
                return Err("Fresh robots verification refused the request".into());
            };
            let path = url
                .query()
                .map_or_else(|| url.path().to_owned(), |q| format!("{}?{q}", url.path()));
            if robots.allows(&path) {
                Ok(())
            } else {
                Err("Robots disallows this download".into())
            }
        })
    }
}
