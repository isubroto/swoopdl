//! Bounded discovery. Media bodies are never captured or published by exploration.
use crate::{crawl::CrawlTransport, robots::Robots, *};
use sdl_engine::crawl_fetch::{
    BodyBudget, CrawlAccess, CrawlLimits, CrawlRequest, fetch_crawl, fetch_crawl_page, probe_crawl,
};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone, Debug)]
pub enum ExploreEvent {
    PageVisited(Url),
    CandidateFound(Box<CandidateMetadata>),
    Skipped(Url, String),
    Stopped { bytes: u64, limited: bool },
}

struct Access {
    config: ProjectConfig,
    network: Arc<dyn CrawlAccess>,
    robots: Arc<RwLock<BTreeMap<String, Robots>>>,
}
impl CrawlAccess for Access {
    fn check_url(&self, url: &Url) -> Result<(), String> {
        self.network.check_url(url)?;
        let page = evaluate(&self.config, url, ResourceKind::Page, 0) == Decision::Allow;
        let file = evaluate(&self.config, url, ResourceKind::Asset, 0) == Decision::Allow;
        if !page && !file {
            return Err("URL is outside exploration scope".into());
        }
        let rules = self.robots.read().map_err(|_| "Robots state unavailable")?;
        let rule = rules
            .get(&url.origin().ascii_serialization())
            .ok_or("Origin has no verified robots policy")?;
        let path = url
            .query()
            .map_or_else(|| url.path().to_owned(), |q| format!("{}?{q}", url.path()));
        if !rule.allows(&path) {
            return Err("Robots disallows the destination".into());
        }
        Ok(())
    }
    fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), String> {
        self.network.check_ip(url, ip)
    }
    fn system_direct(&self, url: &Url) -> Result<bool, String> {
        self.network.system_direct(url)
    }
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("body"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

pub async fn explore(
    config: &ProjectConfig,
    transport: CrawlTransport,
    cancel: CancellationToken,
) -> io::Result<Vec<ExploreEvent>> {
    let mut state = crate::catalog::FileProject::new(config.clone());
    explore_catalog(&mut state, None, transport, cancel, None).await
}

pub async fn explore_catalog(
    state: &mut crate::catalog::FileProject,
    store: Option<&crate::catalog::CatalogStore>,
    transport: CrawlTransport,
    cancel: CancellationToken,
    observe: Option<&(dyn Fn(&crate::catalog::FileProject) + Send + Sync)>,
) -> io::Result<Vec<ExploreEvent>> {
    state.validate()?;
    let config = &state.config.clone();
    state.explore_status = crate::catalog::ExploreStatus::Exploring;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let scratch =
        Scratch(std::env::temp_dir().join(format!("swoopdl-scan-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&scratch.0)?;
    let body_path = scratch.0.join("body");
    let budget = Arc::new(
        BodyBudget::new(config.limits.body_bytes, state.scan_bytes)
            .with_interval(Duration::from_millis(config.limits.interval_ms)),
    );
    let limits = CrawlLimits {
        body_bytes: config.limits.parse_bytes as u64,
        redirects: config.limits.redirects,
        timeout: Duration::from_secs(config.limits.timeout_secs),
    };
    let robots = Arc::new(RwLock::new(BTreeMap::new()));
    let access = Arc::new(Access {
        config: config.clone(),
        network: transport.access.clone(),
        robots: robots.clone(),
    });
    let mut pending: VecDeque<_> = state
        .frontier
        .iter()
        .map(|p| (p.url.clone(), p.depth, p.source_page.clone()))
        .collect();
    let mut queued: std::collections::BTreeSet<_> =
        pending.iter().map(|(url, _, _)| UrlKey::of(url)).collect();
    let mut seen = state.seen.clone();
    let mut events = Vec::new();
    let mut limited = false;
    let mut failed = false;
    let mut applied = 0;
    loop {
        for event in &events[applied..] {
            state.apply(event);
        }
        applied = events.len();
        state.frontier = pending
            .iter()
            .map(|(url, depth, source_page)| crate::catalog::ScanPending {
                url: url.clone(),
                depth: *depth,
                source_page: source_page.clone(),
            })
            .collect();
        state.seen = seen.clone();
        state.scan_bytes = budget.consumed();
        state.reserved_bytes = 0;
        if let Some(store) = store {
            store.checkpoint(state)?;
        }
        if let Some(observe) = observe {
            observe(state);
        }
        if cancel.is_cancelled() {
            break;
        }
        let Some((url, depth, source_page)) = pending.pop_front() else {
            break;
        };
        queued.remove(&UrlKey::of(&url));
        if seen.contains(&UrlKey::of(&url)) {
            continue;
        }
        if seen.len() >= config.limits.urls || budget.remaining() == 0 {
            limited = true;
            break;
        }
        // Save the current frontier before I/O. A crash repeats only this URL; merge is idempotent.
        state.reserved_bytes = budget
            .remaining()
            .min(config.limits.parse_bytes as u64 + 32768 + config.limits.robots_bytes);
        if let Some(store) = store {
            store.checkpoint(state)?;
        }
        seen.insert(UrlKey::of(&url));
        let role = crate::classify::link_role(&url);
        let page = evaluate(config, &url, ResourceKind::Page, depth) == Decision::Allow;
        let file = evaluate(config, &url, ResourceKind::Asset, depth) == Decision::Allow;
        // A parent directory is navigation, never an out-of-scope file probe.
        let page_scope = evaluate(config, &url, ResourceKind::Page, 0) == Decision::Allow;
        if (role != LinkRole::File && !page_scope) || (!page_scope && !file) {
            events.push(ExploreEvent::Skipped(url, "Scope or page depth".into()));
            continue;
        }
        let origin = url.origin().ascii_serialization();
        let known = robots
            .read()
            .map_err(|_| io::Error::other("Robots state unavailable"))?
            .contains_key(&origin);
        if !known {
            let robot_url =
                Url::parse(&format!("{origin}/robots.txt")).map_err(io::Error::other)?;
            let _ = std::fs::remove_file(&body_path);
            let mut robot_limits = limits.clone();
            robot_limits.body_bytes = config.limits.robots_bytes;
            // Robots must stay at the queried origin, including every redirect.
            struct RobotAccess(Arc<dyn CrawlAccess>, String);
            impl CrawlAccess for RobotAccess {
                fn check_url(&self, u: &Url) -> Result<(), String> {
                    self.0.check_url(u)?;
                    if u.origin().ascii_serialization() == self.1 {
                        Ok(())
                    } else {
                        Err("Cross-origin robots redirect".into())
                    }
                }
                fn check_ip(&self, u: &Url, ip: IpAddr) -> Result<(), String> {
                    self.0.check_ip(u, ip)
                }
                fn system_direct(&self, u: &Url) -> Result<bool, String> {
                    self.0.system_direct(u)
                }
            }
            let response = fetch_crawl(
                CrawlRequest {
                    url: robot_url,
                    headers: Default::default(),
                    destination: body_path.clone(),
                },
                transport.options.clone(),
                robot_limits,
                budget.clone(),
                Arc::new(RobotAccess(transport.access.clone(), origin.clone())),
                cancel.clone(),
            )
            .await;
            if cancel.is_cancelled() {
                seen.remove(&UrlKey::of(&url));
                pending.push_front((url, depth, source_page));
                continue;
            }
            let rule = match response {
                Err(
                    sdl_engine::crawl_fetch::CrawlFetchError::Budget
                    | sdl_engine::crawl_fetch::CrawlFetchError::ResourceLimit,
                ) => {
                    events.push(ExploreEvent::Skipped(
                        url,
                        "Robots lookup stopped: body-byte or robots size limit reached".into(),
                    ));
                    limited = true;
                    break;
                }
                Err(error) => {
                    events.push(ExploreEvent::Skipped(
                        url,
                        format!(
                            "Robots lookup failed: {}",
                            crate::crawl::fetch_failure(&error)
                        ),
                    ));
                    failed = true;
                    break;
                }
                Ok(r) if r.status.is_success() => {
                    Robots::parse(&std::fs::read_to_string(&body_path)?)
                }
                Ok(r)
                    if r.status.is_client_error()
                        && !matches!(r.status.as_u16(), 401 | 403 | 408 | 429) =>
                {
                    Robots::default()
                }
                Ok(r) => {
                    events.push(ExploreEvent::Skipped(
                        url,
                        format!("Robots lookup failed: HTTP {}", r.status.as_u16()),
                    ));
                    failed = true;
                    break;
                }
            };
            robots
                .write()
                .map_err(|_| io::Error::other("Robots state unavailable"))?
                .insert(origin.clone(), rule);
        }
        if let Err(reason) = access.check_url(&url) {
            events.push(ExploreEvent::Skipped(url, reason));
            continue;
        }
        let mut headers = if transport.credential_origin.as_deref() == Some(&origin) {
            transport.headers.clone()
        } else {
            Default::default()
        };
        if source_page.origin() == url.origin()
            && let Ok(value) = reqwest::header::HeaderValue::from_str(source_page.as_str())
        {
            headers.insert(reqwest::header::REFERER, value);
        }
        let response = match probe_crawl(
            url.clone(),
            headers.clone(),
            transport.options.clone(),
            limits.clone(),
            budget.clone(),
            access.clone(),
            cancel.clone(),
        )
        .await
        {
            Ok(r) if r.status.is_success() => r,
            Ok(r) => {
                events.push(ExploreEvent::Skipped(
                    url,
                    format!("HTTP {}", r.status.as_u16()),
                ));
                continue;
            }
            Err(_) if cancel.is_cancelled() => {
                seen.remove(&UrlKey::of(&url));
                pending.push_front((url, depth, source_page));
                continue;
            }
            Err(error) => {
                events.push(ExploreEvent::Skipped(
                    url,
                    crate::crawl::fetch_failure(&error),
                ));
                continue;
            }
        };
        if crate::classify::is_navigation(&response.mime) {
            if !page
                || evaluate(config, &response.final_url, ResourceKind::Page, depth)
                    != Decision::Allow
            {
                events.push(ExploreEvent::Skipped(url, "Page depth or scope".into()));
                continue;
            }
            // An explicitly named video returning HTML is a login/error, not navigation.
            if role == LinkRole::File
                && !(url.path().ends_with(".css")
                    && crate::media_type(&response.mime) == "text/css")
            {
                events.push(ExploreEvent::Skipped(
                    url,
                    "File returned HTML; session may be required".into(),
                ));
                continue;
            }
            let _ = std::fs::remove_file(&body_path);
            let response = match fetch_crawl_page(
                CrawlRequest {
                    url: url.clone(),
                    headers,
                    destination: body_path.clone(),
                },
                transport.options.clone(),
                limits.clone(),
                budget.clone(),
                access.clone(),
                cancel.clone(),
            )
            .await
            {
                Ok(r) if r.status.is_success() => r,
                _ if cancel.is_cancelled() => {
                    seen.remove(&UrlKey::of(&url));
                    pending.push_front((url, depth, source_page));
                    continue;
                }
                _ => {
                    events.push(ExploreEvent::Skipped(
                        url,
                        "Navigation fetch failed or changed type".into(),
                    ));
                    continue;
                }
            };
            let body = std::fs::read(&body_path)?;
            let (links, truncated) = crate::discover::discover_bounded(
                &body,
                &response.mime,
                &response.final_url,
                config.limits.urls,
            )
            .map_err(io::Error::other)?;
            events.push(ExploreEvent::PageVisited(response.final_url.clone()));
            limited |= truncated;
            for link in links {
                if pending.len() >= config.limits.urls {
                    limited = true;
                    break;
                }
                if seen.contains(&UrlKey::of(&link.url)) || !queued.insert(UrlKey::of(&link.url)) {
                    continue;
                }
                pending.push_back((
                    link.url,
                    depth.saturating_add(1),
                    response.final_url.clone(),
                ));
            }
        } else if file
            && evaluate(config, &response.final_url, ResourceKind::Asset, depth) == Decision::Allow
            && crate::policy::save_allowed(
                config,
                &response.final_url,
                ResourceKind::Asset,
                &response.mime,
            )
        {
            let name = response
                .final_url
                .path_segments()
                .and_then(|mut p| p.next_back())
                .filter(|n| !n.is_empty())
                .unwrap_or("download")
                .to_owned();
            events.push(ExploreEvent::CandidateFound(Box::new(CandidateMetadata {
                url,
                final_url: response.final_url,
                name,
                mime: (!response.mime.is_empty()).then_some(response.mime),
                size: response.length,
                source_page,
            })));
        } else {
            events.push(ExploreEvent::Skipped(url, "File filter".into()));
        }
    }
    events.push(ExploreEvent::Stopped {
        bytes: budget.consumed(),
        limited,
    });
    for event in &events[applied..] {
        state.apply(event);
    }
    if !cancel.is_cancelled() && !limited && state.frontier.is_empty() {
        state.explore_status = crate::catalog::ExploreStatus::Completed;
    }
    if failed {
        state.explore_status = crate::catalog::ExploreStatus::Failed;
        state.next_run = None;
    }
    state.reserved_bytes = 0;
    if let Some(store) = store {
        store.checkpoint(state)?;
    }
    if let Some(observe) = observe {
        observe(state);
    }
    Ok(events)
}
