//! Durable breadth-first capture. One request at a time stays below all request caps.
use crate::{discover::discover_bounded, robots::Robots, store::ProjectStore, *};
use reqwest::header::{HeaderMap, IF_MODIFIED_SINCE, IF_NONE_MATCH};
use sdl_engine::crawl_fetch::{
    BodyBudget, CrawlAccess, CrawlFetchError, CrawlLimits, CrawlRequest, CrawlResponse, fetch_crawl,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read},
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use url::Url;

#[cfg(test)]
mod stack_tests {
    use super::*;
    #[test]
    fn capture_future_fits_small_thread_stacks() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig::new(
            "Stack",
            "https://example.test/".parse().unwrap(),
            dir.path().join("output"),
        );
        let state = ProjectSnapshot::new(config.clone());
        let store =
            ProjectStore::open(dir.path(), &crate::ProjectId::parse("stack").unwrap()).unwrap();
        let (_commands, rx) = mpsc::channel(2);
        let (tx, _) = mpsc::channel(2);
        let future = run(
            config,
            state,
            store,
            CrawlTransport::new(
                Default::default(),
                Arc::new(sdl_engine::crawl_fetch::NetworkAccess::public()),
            ),
            rx,
            tx,
        );
        let bytes = std::mem::size_of_val(&future);
        assert!(
            bytes <= 32 * 1024,
            "Capture future occupies {bytes} stack bytes"
        );
    }
}
pub type CrawlError = io::Error;
#[derive(Clone, Debug)]
pub enum CrawlCommand {
    Pause,
    Cancel,
}
#[derive(Clone, Debug)]
pub struct CrawlEvent {
    pub status: ProjectStatus,
    pub saved: usize,
    pub pending: usize,
    pub bytes: u64,
    pub reason: Option<String>,
}
#[derive(Clone)]
pub struct CrawlTransport {
    pub options: sdl_engine::Options,
    pub access: Arc<dyn CrawlAccess>,
    /// Transient secrets, never serialized or included in events/errors.
    pub headers: HeaderMap,
    pub credential_origin: Option<String>,
}
impl CrawlTransport {
    pub fn new(options: sdl_engine::Options, access: Arc<dyn CrawlAccess>) -> Self {
        Self {
            options,
            access,
            headers: HeaderMap::new(),
            credential_origin: None,
        }
    }
}
struct ScopedAccess {
    network: Arc<dyn CrawlAccess>,
    config: ProjectConfig,
    kind: ResourceKind,
    depth: u16,
    seen: Arc<Mutex<BTreeSet<UrlKey>>>,
    robots: BTreeMap<String, Robots>,
}
impl CrawlAccess for ScopedAccess {
    fn check_url(&self, url: &Url) -> Result<(), String> {
        self.network.check_url(url)?;
        if let Decision::Skip(reason) = evaluate(&self.config, url, self.kind, self.depth) {
            return Err(format!("URL rejected by capture policy: {reason:?}"));
        }
        let robots = self
            .robots
            .get(&url.origin().ascii_serialization())
            .ok_or("Redirect origin has no verified robots policy")?;
        if !robots.allows(&request_path(url)) {
            return Err("Robots disallows the destination".into());
        }
        let mut seen = self.seen.lock().map_err(|_| "Capture state unavailable")?;
        let key = UrlKey::of(url);
        if !seen.contains(&key) && seen.len() >= self.config.limits.urls {
            return Err("URL limit reached".into());
        }
        seen.insert(key);
        Ok(())
    }
    fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), String> {
        self.network.check_ip(url, ip)
    }
}
fn request_path(url: &Url) -> String {
    match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().into(),
    }
}
fn emit(state: &ProjectSnapshot, events: &mpsc::Sender<CrawlEvent>, reason: Option<String>) {
    let _ = events.try_send(CrawlEvent {
        status: state.status,
        saved: state.resources.len(),
        pending: state.pending.len(),
        bytes: state.consumed_bytes,
        reason,
    });
}
fn report(state: &mut ProjectSnapshot, url: &Url, reason: impl Into<String>) {
    if state.reports.len() < state.config.limits.urls {
        state.reports.push(ReportEntry {
            url: url.clone(),
            reason: reason.into(),
        });
    }
}
async fn sleep(delay: Duration, cancel: &CancellationToken) -> bool {
    if let Some(at) = tokio::time::Instant::now().checked_add(delay) {
        tokio::select! { _ = cancel.cancelled() => false, _ = tokio::time::sleep_until(at) => true }
    } else {
        cancel.cancelled().await;
        false
    }
}
fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

/// Save a worst-case reservation before any network read. Power loss cannot refund it.
struct FetchSpec {
    url: Url,
    headers: HeaderMap,
    cap: u64,
    access: Arc<dyn CrawlAccess>,
}
async fn request(
    state: &mut ProjectSnapshot,
    store: &ProjectStore,
    transport: &CrawlTransport,
    spec: FetchSpec,
    budget: Arc<BodyBudget>,
    cancel: &CancellationToken,
) -> io::Result<Result<CrawlResponse, CrawlFetchError>> {
    let FetchSpec {
        url,
        headers,
        cap,
        access,
    } = spec;
    let wait = state
        .next_request_after
        .unwrap_or_default()
        .saturating_sub(unix_millis());
    if !sleep(Duration::from_millis(wait), cancel).await {
        return Ok(Err(CrawlFetchError::Cancelled));
    }
    let root = state.config.destination.clone();
    let destination = paths::checked_path(&root, std::path::Path::new("temporary/request.body"))?;
    if destination.exists() {
        fs::remove_file(&destination)?;
    }
    state.reserved_bytes = budget.remaining();
    store.checkpoint(state)?;
    let limits = CrawlLimits {
        body_bytes: cap,
        redirects: state.config.limits.redirects,
        timeout: Duration::from_secs(state.config.limits.timeout_secs),
    };
    let result = fetch_crawl(
        CrawlRequest {
            url,
            headers,
            destination,
        },
        transport.options.clone(),
        limits,
        budget.clone(),
        access,
        cancel.clone(),
    )
    .await;
    state.consumed_bytes = budget.consumed();
    state.reserved_bytes = 0;
    let delay = result
        .as_ref()
        .ok()
        .and_then(|r| r.retry_after)
        .unwrap_or_default()
        .max(Duration::from_millis(state.config.limits.interval_ms));
    let deadline = unix_millis().saturating_add(delay.as_millis().min(u64::MAX as u128) as u64);
    state.next_request_after = Some(
        state
            .next_request_after
            .unwrap_or_default()
            .max(deadline)
            .max(budget.next_request_after()),
    );
    store.checkpoint(state)?;
    Ok(result)
}
pub async fn run(
    config: ProjectConfig,
    mut state: ProjectSnapshot,
    store: ProjectStore,
    transport: CrawlTransport,
    mut commands: mpsc::Receiver<CrawlCommand>,
    events: mpsc::Sender<CrawlEvent>,
) -> Result<ProjectSnapshot, CrawlError> {
    config.validate().map_err(io::Error::other)?;
    state.config = config;
    if state.status != ProjectStatus::Draft {
        let destination = state.config.destination.clone();
        state = crate::store::reconcile(state, &destination)?;
    }
    let cancel = CancellationToken::new();
    let result = {
        // Keep the large transport future off small native thread stacks.
        let mut worker = Box::pin(run_inner(&mut state, &store, &transport, &events, &cancel));
        tokio::select! { biased;
          command = commands.recv() => {
              if command.is_some() { cancel.cancel(); }
              worker.await
          },
          result = &mut worker => result,
        }
    };
    result?;
    Ok(state)
}
async fn run_inner(
    state: &mut ProjectSnapshot,
    store: &ProjectStore,
    transport: &CrawlTransport,
    events: &mpsc::Sender<CrawlEvent>,
    cancel: &CancellationToken,
) -> io::Result<()> {
    if state.status != ProjectStatus::Draft && !state.config.destination.is_dir() {
        return Err(io::Error::other(
            "Capture destination is missing. Reconnect it before resuming.",
        ));
    }
    let mut limited = state.limit_reached || state.status == ProjectStatus::Limited;
    state.limit_reached = limited;
    store.prepare_output(&state.config.destination)?;
    state.status = ProjectStatus::Running;
    state.next_request_after = Some(
        state
            .next_request_after
            .unwrap_or_default()
            .max(unix_millis().saturating_add(state.config.limits.interval_ms)),
    );
    store.checkpoint(state)?;
    let budget = Arc::new(BodyBudget::new(
        state.config.limits.body_bytes,
        state.consumed_bytes,
    ));
    let seen = Arc::new(Mutex::new(state.seen.clone()));
    let mut robots: BTreeMap<String, Robots> = BTreeMap::new();
    while let Some(pending) = state.pending.front().cloned() {
        if cancel.is_cancelled() {
            break;
        }
        if budget.remaining() == 0 {
            limited = true;
            report(state, &pending.url, "Body-byte limit reached");
            break;
        }
        let origin = pending.url.origin().ascii_serialization();
        if !robots.contains_key(&origin) {
            let url = Url::parse(&format!("{origin}/robots.txt")).map_err(io::Error::other)?;
            let mut policy = Robots::blocked();
            for attempt in 0..state.config.limits.attempts {
                let result = request(
                    state,
                    store,
                    transport,
                    FetchSpec {
                        url: url.clone(),
                        headers: HeaderMap::new(),
                        cap: state.config.limits.robots_bytes,
                        access: transport.access.clone(),
                    },
                    budget.clone(),
                    cancel,
                )
                .await?;
                match result {
                    Ok(response) => {
                        let status = response.status.as_u16();
                        if (200..300).contains(&status) {
                            if let Some(path) = response.body {
                                let bytes = fs::read(&path)?;
                                policy = Robots::parse(&String::from_utf8_lossy(&bytes));
                                fs::remove_file(path)?;
                            }
                            break;
                        }
                        if (400..500).contains(&status) && !matches!(status, 401 | 403 | 408 | 429)
                        {
                            policy = Robots::default();
                            break;
                        }
                        if matches!(status, 401 | 403) {
                            break;
                        }
                        if attempt + 1 < state.config.limits.attempts
                            && !sleep(
                                response
                                    .retry_after
                                    .unwrap_or_default()
                                    .max(Duration::from_secs(1 << attempt)),
                                cancel,
                            )
                            .await
                        {
                            break;
                        }
                    }
                    Err(CrawlFetchError::Cancelled) => break,
                    Err(_) => {
                        if attempt + 1 < state.config.limits.attempts
                            && !sleep(Duration::from_secs(1 << attempt), cancel).await
                        {
                            break;
                        }
                    }
                }
            }
            robots.insert(origin.clone(), policy);
        }
        if cancel.is_cancelled() {
            break;
        }
        let access = Arc::new(ScopedAccess {
            network: transport.access.clone(),
            config: state.config.clone(),
            kind: pending.kind,
            depth: pending.depth,
            seen: seen.clone(),
            robots: robots.clone(),
        });
        if let Err(reason) = access.check_url(&pending.url) {
            report(state, &pending.url, reason);
            state.pending.pop_front();
            continue;
        }
        let key = UrlKey::of(&pending.url);
        let mut headers = if transport.credential_origin.as_deref() == Some(origin.as_str()) {
            transport.headers.clone()
        } else {
            HeaderMap::new()
        };
        if let Some(previous) = state.resources.get(&key)
            && store.verified_body(state, &key).is_ok()
        {
            if let Some(v) = previous.etag.as_ref().and_then(|v| v.parse().ok()) {
                headers.insert(IF_NONE_MATCH, v);
            }
            if let Some(v) = previous.last_modified.as_ref().and_then(|v| v.parse().ok()) {
                headers.insert(IF_MODIFIED_SINCE, v);
            }
        }
        let mut response = None;
        for attempt in 0..state.config.limits.attempts {
            let result = request(
                state,
                store,
                transport,
                FetchSpec {
                    url: pending.url.clone(),
                    headers: headers.clone(),
                    cap: state.config.limits.resource_bytes,
                    access: access.clone(),
                },
                budget.clone(),
                cancel,
            )
            .await?;
            match result {
                Ok(r) if r.status.as_u16() == 304 => {
                    if store.verified_body(state, &key).is_ok() {
                        response = Some(r);
                        break;
                    }
                    headers.remove(IF_NONE_MATCH);
                    headers.remove(IF_MODIFIED_SINCE);
                    if attempt + 1 == state.config.limits.attempts {
                        report(state, &pending.url, "304 without a verified saved body");
                    }
                }
                Ok(r) if matches!(r.status.as_u16(), 408 | 429 | 500..=599) => {
                    if attempt + 1 == state.config.limits.attempts {
                        report(
                            state,
                            &pending.url,
                            format!("HTTP {} after retry limit", r.status.as_u16()),
                        );
                    } else if !sleep(
                        r.retry_after
                            .unwrap_or_default()
                            .max(Duration::from_secs(1 << attempt)),
                        cancel,
                    )
                    .await
                    {
                        break;
                    }
                }
                Ok(r) => {
                    response = Some(r);
                    break;
                }
                Err(CrawlFetchError::Budget | CrawlFetchError::ResourceLimit) => {
                    limited = true;
                    report(
                        state,
                        &pending.url,
                        "Body-byte or resource size limit reached",
                    );
                    break;
                }
                Err(CrawlFetchError::Cancelled) => break,
                Err(CrawlFetchError::Http(_) | CrawlFetchError::Body(_)) => {
                    if attempt + 1 == state.config.limits.attempts {
                        report(
                            state,
                            &pending.url,
                            "Network request failed after retry limit",
                        );
                    } else if !sleep(Duration::from_secs(1 << attempt), cancel).await {
                        break;
                    }
                }
                Err(CrawlFetchError::Access(reason)) if reason == "URL limit reached" => {
                    limited = true;
                    report(state, &pending.url, "URL limit reached");
                    break;
                }
                Err(CrawlFetchError::Access(reason)) => {
                    report(state, &pending.url, reason);
                    break;
                }
                Err(CrawlFetchError::Io(_)) => {
                    report(
                        state,
                        &pending.url,
                        "Capture destination could not be written",
                    );
                    break;
                }
                Err(CrawlFetchError::Setup(_)) => {
                    report(state, &pending.url, "Network settings could not be applied");
                    break;
                }
            }
        }
        state.seen = seen
            .lock()
            .map_err(|_| io::Error::other("Capture state unavailable"))?
            .clone();
        if cancel.is_cancelled() {
            break;
        }
        if let Some(r) = response {
            if r.status.is_success() {
                if let Some(path) = r.body {
                    let mut file = fs::File::open(&path)?;
                    let mut hash = Sha256::new();
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        let n = file.read(&mut buffer)?;
                        if n == 0 {
                            break;
                        }
                        hash.update(&buffer[..n]);
                    }
                    let hash = hex::encode(hash.finalize());
                    let raw = store.raw_path(&state.config.destination, &hash)?;
                    if raw.exists() {
                        fs::remove_file(&path)?;
                    } else {
                        fs::rename(&path, &raw)?;
                    }
                    let mime = crate::media_type(&r.mime);
                    let document = matches!(
                        mime.as_str(),
                        "text/html" | "application/xhtml+xml" | "text/css" | "image/svg+xml"
                    );
                    let html_asset = pending.kind == ResourceKind::Asset
                        && matches!(mime.as_str(), "text/html" | "application/xhtml+xml");
                    let kind = if html_asset {
                        ResourceKind::Page
                    } else {
                        pending.kind
                    };
                    let depth = pending.depth.saturating_add(u16::from(html_asset));
                    if document
                        && fs::metadata(&raw)?.len() > state.config.limits.parse_bytes as u64
                    {
                        report(
                            state,
                            &pending.url,
                            "Parse limit: document was not published",
                        );
                        limited = true;
                    } else if html_asset
                        && evaluate(&state.config, &r.final_url, kind, depth) != Decision::Allow
                    {
                        report(
                            state,
                            &pending.url,
                            "Asset response is HTML outside page scope",
                        );
                    } else {
                        state.resources.insert(
                            key.clone(),
                            ResourceRecord {
                                url: pending.url.clone(),
                                final_url: r.final_url,
                                kind,
                                depth,
                                mime: r.mime,
                                raw_hash: hash,
                                etag: r.etag,
                                last_modified: r.last_modified,
                                aliases: r.aliases,
                            },
                        );
                    }
                }
            } else if r.status.as_u16() != 304 {
                report(state, &pending.url, format!("HTTP {}", r.status.as_u16()));
            }
            if let Some(record) = state.resources.get(&key).cloned() {
                let raw = store.raw_path(&state.config.destination, &record.raw_hash)?;
                let parseable = matches!(
                    crate::media_type(&record.mime).as_str(),
                    "text/html" | "application/xhtml+xml" | "text/css"
                );
                if parseable && fs::metadata(&raw)?.len() <= state.config.limits.parse_bytes as u64
                {
                    let body = fs::read(raw)?;
                    match discover_bounded(
                        &body,
                        &record.mime,
                        &record.final_url,
                        state.config.limits.urls,
                    ) {
                        Ok((links, truncated)) => {
                            if truncated {
                                limited = true;
                                report(state, &pending.url, "Discovery URL limit reached");
                            }
                            for link in links {
                                let link_key = UrlKey::of(&link.url);
                                if state.seen.contains(&link_key) {
                                    continue;
                                }
                                if state.seen.len() >= state.config.limits.urls {
                                    limited = true;
                                    break;
                                }
                                state.seen.insert(link_key.clone());
                                seen.lock()
                                    .map_err(|_| io::Error::other("Capture state unavailable"))?
                                    .insert(link_key);
                                let depth = record
                                    .depth
                                    .saturating_add(u16::from(link.kind == ResourceKind::Page));
                                match evaluate(&state.config, &link.url, link.kind, depth) {
                                    Decision::Allow => state.pending.push_back(Pending {
                                        url: link.url,
                                        kind: link.kind,
                                        depth,
                                    }),
                                    Decision::Skip(reason) => {
                                        report(state, &link.url, format!("{reason:?}"))
                                    }
                                }
                            }
                        }
                        Err(_) => report(state, &pending.url, "Document could not be parsed"),
                    }
                } else if parseable {
                    report(state, &pending.url, "Parse limit: document was not parsed");
                    limited = true;
                }
            }
        }
        state.pending.pop_front();
        state.limit_reached = limited;
        store.checkpoint(state)?;
        emit(state, events, None);
        if !sleep(
            Duration::from_millis(state.config.limits.interval_ms.saturating_sub(1000)),
            cancel,
        )
        .await
        {
            break;
        }
    }
    state.consumed_bytes = budget.consumed();
    state.reserved_bytes = 0;
    limited |= budget.remaining() == 0 || state.seen.len() >= state.config.limits.urls;
    state.limit_reached = limited;
    state.status = if cancel.is_cancelled() {
        ProjectStatus::Paused
    } else if limited {
        ProjectStatus::Limited
    } else if state
        .resources
        .contains_key(&UrlKey::of(&state.config.seed))
    {
        ProjectStatus::Completed
    } else {
        ProjectStatus::Failed
    };
    if matches!(
        state.status,
        ProjectStatus::Completed | ProjectStatus::Limited
    ) && state
        .resources
        .contains_key(&UrlKey::of(&state.config.seed))
    {
        let mut candidate = state.clone();
        let publication_store = store.clone();
        let publication_cancel = cancel.clone();
        let (published, result) = tokio::task::spawn_blocking(move || {
            let result = crate::rewrite::publish_generation(
                &publication_store,
                &mut candidate,
                &publication_cancel,
            );
            (candidate, result)
        })
        .await
        .map_err(io::Error::other)?;
        *state = published;
        if let Err(error) = result {
            if error.kind() == io::ErrorKind::Interrupted && cancel.is_cancelled() {
                state.status = ProjectStatus::Paused;
                store.checkpoint(state)?;
                emit(state, events, None);
                return Ok(());
            }
            let cap = error.kind() == io::ErrorKind::FileTooLarge;
            state.limit_reached |= cap;
            state.status = if cap {
                ProjectStatus::Limited
            } else {
                ProjectStatus::Failed
            };
            let seed = state.config.seed.clone();
            report(
                state,
                &seed,
                if cap {
                    "Rewritten document output limit reached"
                } else {
                    "Offline publication failed; the previous copy was retained"
                },
            );
            store.checkpoint(state)?;
            emit(
                state,
                events,
                Some("Offline publication could not finish".into()),
            );
            if !cap {
                return Err(error);
            }
        }
    } else {
        store.checkpoint(state)?;
    }
    emit(state, events, None);
    Ok(())
}
