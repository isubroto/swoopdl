//! Crawl-only transport: checked DNS answers, manual redirects and shared body budgets.
//! Ordinary download redirect and retry behavior is intentionally independent.
use crate::{Options, Proxy, limiter::Limiter};
use futures::StreamExt;
use reqwest::{
    StatusCode,
    dns::{Addrs, Name, Resolve, Resolving},
    header::{self, HeaderMap},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, ReadBuf};
use tokio_util::io::StreamReader;
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum CrawlFetchError {
    #[error("The crawl body-byte limit was reached")]
    Budget,
    #[error("The resource size limit was reached")]
    ResourceLimit,
    #[error("The request was cancelled")]
    Cancelled,
    #[error("Crawl access was refused: {0}")]
    Access(String),
    #[error("The server request failed")]
    Http(#[source] reqwest::Error),
    #[error("The response body could not be decoded")]
    Body(#[source] std::io::Error),
    #[error("The capture file could not be written: {0}")]
    Io(#[from] std::io::Error),
    #[error("The network configuration could not be applied")]
    Setup(#[source] crate::Error),
}

#[cfg(test)]
mod decode_budget_tests {
    use super::*;
    #[test]
    fn transport_future_fits_small_thread_stacks() {
        let future = fetch_crawl(
            CrawlRequest {
                url: "https://example.test/".parse().unwrap(),
                headers: HeaderMap::new(),
                destination: PathBuf::from("unused"),
            },
            Options::default(),
            CrawlLimits::default(),
            Arc::new(BodyBudget::new(1024, 0)),
            Arc::new(NetworkAccess::public()),
            CancellationToken::new(),
        );
        let bytes = std::mem::size_of_val(&future);
        assert!(
            bytes <= 32 * 1024,
            "Transport future occupies {bytes} stack bytes"
        );
    }
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::io::{AsyncRead, ReadBuf};

    struct Meter {
        left: usize,
        produced: Arc<AtomicU64>,
        yielded: bool,
    }
    impl AsyncRead for Meter {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let me = self.get_mut();
            if !me.yielded {
                me.yielded = true;
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            me.yielded = false;
            let n = me.left.min(buf.remaining());
            buf.put_slice(&vec![0u8; n]);
            me.left -= n;
            me.produced.fetch_add(n as u64, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
    #[tokio::test]
    async fn decoding_reserves_budget_before_reading() {
        let produced = Arc::new(AtomicU64::new(0));
        let mut reader = Meter {
            left: 65536,
            produced: produced.clone(),
            yielded: false,
        };
        let budget = Arc::new(BodyBudget::new(512, 0));
        let result = copy_body(
            &mut reader,
            &mut None,
            BodyCopy {
                cap: 1 << 20,
                expected: None,
                budget: budget.clone(),
            },
        )
        .await;
        assert!(matches!(result, Err(CrawlFetchError::Budget)));
        assert_eq!(produced.load(Ordering::SeqCst), 512);
        assert_eq!(budget.consumed(), 512);
    }
    #[tokio::test]
    async fn parallel_decoders_refund_unused_reservations_without_overreading() {
        let produced = Arc::new(AtomicU64::new(0));
        let budget = Arc::new(BodyBudget::new(32, 0));
        let mut tasks = Vec::new();
        for _ in 0..4 {
            let (produced, budget) = (produced.clone(), budget.clone());
            tasks.push(tokio::spawn(async move {
                let mut reader = Meter {
                    left: 10,
                    produced,
                    yielded: false,
                };
                copy_body(
                    &mut reader,
                    &mut None,
                    BodyCopy {
                        cap: 100,
                        expected: None,
                        budget,
                    },
                )
                .await
            }));
        }
        let completed =
            tokio::time::timeout(Duration::from_secs(2), futures::future::join_all(tasks))
                .await
                .unwrap();
        assert_eq!(
            completed
                .into_iter()
                .filter(|r| r.as_ref().unwrap().is_ok())
                .count(),
            3
        );
        assert_eq!(produced.load(Ordering::SeqCst), 32);
        assert_eq!(budget.consumed(), 32);
    }
    #[tokio::test]
    async fn cancelling_a_pending_decode_refunds_known_unused_bytes() {
        let (mut reader, _writer) = tokio::io::duplex(64);
        let budget = Arc::new(BodyBudget::new(512, 0));
        let copy_budget = budget.clone();
        let task = tokio::spawn(async move {
            copy_body(
                &mut reader,
                &mut None,
                BodyCopy {
                    cap: 1024,
                    expected: None,
                    budget: copy_budget,
                },
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while budget.remaining() == 512 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(budget.remaining() < 512);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(budget.consumed(), 0);
        assert_eq!(budget.remaining(), 512);
    }
}
#[derive(Debug)]
pub struct BodyBudget {
    limit: u64,
    accounting: Mutex<BudgetAccounting>,
    changed: tokio::sync::Notify,
    server_after: AtomicU64,
    pacing: tokio::sync::Mutex<BTreeMap<String, tokio::time::Instant>>,
    interval: Duration,
}
#[derive(Debug)]
struct BudgetAccounting {
    consumed: u64,
    reserved: u64,
}
struct BudgetLease {
    budget: Arc<BodyBudget>,
    amount: u64,
    produced: Option<u64>,
}
impl BudgetLease {
    fn commit(&mut self, actual: u64) {
        let amount = self.amount;
        if amount == 0 {
            return;
        }
        {
            let mut state = self.budget.accounting.lock().unwrap();
            state.reserved -= amount;
            state.consumed += actual.min(amount);
        }
        self.amount = 0;
        self.budget.changed.notify_waiters();
    }
}
impl Drop for BudgetLease {
    fn drop(&mut self) {
        // Charge decoded bytes visible in the caller-owned ReadBuf even when a read
        // is cancelled or errors. If a decoder panics before reporting progress,
        // conservatively charge the reservation rather than replenish it.
        self.commit(self.produced.unwrap_or(self.amount));
    }
}
impl BodyBudget {
    pub fn new(limit: u64, consumed: u64) -> Self {
        Self {
            limit,
            accounting: Mutex::new(BudgetAccounting {
                consumed: consumed.min(limit),
                reserved: 0,
            }),
            changed: tokio::sync::Notify::new(),
            server_after: AtomicU64::new(0),
            pacing: Default::default(),
            interval: Duration::from_secs(1),
        }
    }
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval.max(Duration::from_secs(1));
        self
    }
    pub fn consumed(&self) -> u64 {
        self.accounting.lock().unwrap().consumed
    }
    pub fn remaining(&self) -> u64 {
        let state = self.accounting.lock().unwrap();
        self.limit
            .saturating_sub(state.consumed)
            .saturating_sub(state.reserved)
    }
    pub fn charge(&self, bytes: u64) -> Result<(), CrawlFetchError> {
        let mut state = self.accounting.lock().unwrap();
        let free = self
            .limit
            .saturating_sub(state.consumed)
            .saturating_sub(state.reserved);
        state.consumed += bytes.min(free);
        drop(state);
        self.changed.notify_waiters();
        if bytes <= free {
            Ok(())
        } else {
            Err(CrawlFetchError::Budget)
        }
    }
    async fn lease(self: &Arc<Self>, maximum: u64) -> Result<BudgetLease, CrawlFetchError> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let grant = {
                let mut state = self.accounting.lock().unwrap();
                let free = self
                    .limit
                    .saturating_sub(state.consumed)
                    .saturating_sub(state.reserved);
                if free > 0 {
                    let n = free.min(maximum);
                    state.reserved += n;
                    Some(n)
                } else if state.reserved == 0 {
                    return Err(CrawlFetchError::Budget);
                } else {
                    None
                }
            };
            if let Some(amount) = grant {
                return Ok(BudgetLease {
                    budget: self.clone(),
                    amount,
                    produced: None,
                });
            }
            notified.await;
        }
    }
    pub fn next_request_after(&self) -> u64 {
        self.server_after.load(Ordering::SeqCst)
    }
    fn defer(&self, delay: Duration) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let deadline = now.saturating_add(delay.as_millis()).min(u64::MAX as u128) as u64;
        self.server_after.fetch_max(deadline, Ordering::SeqCst);
    }
    async fn pace(&self, url: &Url) {
        let origin = url.origin().ascii_serialization();
        let at = {
            let mut times = self.pacing.lock().await;
            let at = times
                .get(&origin)
                .copied()
                .unwrap_or_else(tokio::time::Instant::now)
                .max(tokio::time::Instant::now());
            times.insert(origin, at + self.interval);
            at
        };
        tokio::time::sleep_until(at).await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        let delay = Duration::from_millis(self.next_request_after().saturating_sub(now));
        if let Some(at) = tokio::time::Instant::now().checked_add(delay) {
            tokio::time::sleep_until(at).await;
        } else {
            futures::future::pending::<()>().await;
        }
    }
}

struct BodyCopy {
    budget: Arc<BodyBudget>,
    cap: u64,
    expected: Option<u64>,
}
async fn copy_body<R: AsyncRead + Unpin>(
    reader: &mut R,
    file: &mut Option<tokio::fs::File>,
    config: BodyCopy,
) -> Result<u64, CrawlFetchError> {
    let mut buffer = [0u8; 64 * 1024];
    let mut size = 0u64;
    while config.expected.is_none_or(|expected| size < expected) {
        let maximum = config
            .cap
            .saturating_sub(size)
            .saturating_add(1)
            .min(buffer.len() as u64);
        let mut lease = config.budget.lease(maximum).await?;
        let mut output = ReadBuf::new(&mut buffer[..lease.amount as usize]);
        let result = futures::future::poll_fn(|cx| {
            let result = Pin::new(&mut *reader).poll_read(cx, &mut output);
            lease.produced = Some(output.filled().len() as u64);
            result
        })
        .await;
        let n = output.filled().len();
        lease.commit(n as u64);
        result.map_err(CrawlFetchError::Body)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or(CrawlFetchError::ResourceLimit)?;
        if size > config.cap {
            return Err(CrawlFetchError::ResourceLimit);
        }
        if let Some(file) = file {
            file.write_all(&buffer[..n]).await?;
        }
    }
    Ok(size)
}

pub trait CrawlAccess: Send + Sync {
    fn check_url(&self, url: &Url) -> Result<(), String>;
    /// Fresh policy checks before each guarded engine request, including retries and segments.
    fn before_request<'a>(
        &'a self,
        url: &'a Url,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move { self.check_url(url) })
    }
    fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), String>;
    fn redirect_limit(&self) -> usize {
        10
    }
    fn check_response(&self, _url: &Url, _mime: &str, _size: Option<u64>) -> Result<(), String> {
        Ok(())
    }
    /// Resolve the effective system route for this hop. Unknown routes fail closed.
    /// Implementations are trusted transport policy, like the address checks above.
    fn system_direct(&self, url: &Url) -> Result<bool, String> {
        crate::net::crawl_system_direct(url)
    }
}
/// Runtime policy for normal engine jobs created by Site Grabber.
#[derive(Clone)]
pub struct CaptureGuard(pub Arc<dyn CrawlAccess>);
impl std::fmt::Debug for CaptureGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CaptureGuard")
    }
}
#[derive(Clone, Debug, Default)]
pub struct NetworkAccess {
    private_origin: Option<String>,
    origins: Option<BTreeSet<String>>,
}
impl NetworkAccess {
    pub fn public() -> Self {
        Self::default()
    }
    pub fn with_private_origin(origin: String) -> Self {
        Self {
            private_origin: Some(origin),
            origins: None,
        }
    }
    pub fn restrict(mut self, origins: BTreeSet<String>) -> Self {
        self.origins = Some(origins);
        self
    }
}
impl CrawlAccess for NetworkAccess {
    fn check_url(&self, u: &Url) -> Result<(), String> {
        if !matches!(u.scheme(), "http" | "https")
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err("Only HTTP(S) without URL credentials is allowed".into());
        }
        if self
            .origins
            .as_ref()
            .is_some_and(|origins| !origins.contains(&u.origin().ascii_serialization()))
        {
            return Err("The destination is outside the allowed origins".into());
        }
        if let Some(host) = u.host_str()
            && let Ok(ip) = host.trim_matches(['[', ']']).parse()
        {
            self.check_ip(u, ip)?;
        }
        Ok(())
    }
    fn check_ip(&self, u: &Url, ip: IpAddr) -> Result<(), String> {
        let ip = match ip {
            IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
            _ => ip,
        };
        let public = match ip {
            IpAddr::V4(v) => {
                let o = v.octets();
                !(v.is_private()
                    || v.is_loopback()
                    || v.is_link_local()
                    || v.is_multicast()
                    || v.is_unspecified()
                    || v.is_broadcast()
                    || v.is_documentation()
                    || o[0] == 0
                    || o[0] >= 240
                    || (o[0] == 100 && (64..=127).contains(&o[1]))
                    || (o[0] == 198 && (18..=19).contains(&o[1]))
                    || (o[0] == 192 && o[1] == 0 && o[2] == 0))
            }
            IpAddr::V6(v) => {
                let s = v.segments();
                (s[0] & 0xe000) == 0x2000
                    && !(s[0] == 0x2001 && s[1] == 0x0db8)
                    && s[0] != 0x2002
                    && !(s[0] == 0x2001 && s[1] < 0x0200)
            }
        };
        if public
            || self.private_origin.as_deref() == Some(u.origin().ascii_serialization().as_str())
        {
            Ok(())
        } else {
            Err(
                "Private or reserved network address requires explicit seed-origin permission"
                    .into(),
            )
        }
    }
}

pub(crate) struct CheckedResolver {
    pub(crate) options: Options,
    pub(crate) url: Url,
    pub(crate) access: Arc<dyn CrawlAccess>,
}
impl Resolve for CheckedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        let (options, url, access) = (self.options.clone(), self.url.clone(), self.access.clone());
        Box::pin(async move {
            // Unlike the download resolver, lookup never probes unvalidated addresses.
            let ips = match options
                .hosts
                .iter()
                .find(|(h, _)| h.eq_ignore_ascii_case(&host))
            {
                Some((_, ips)) => ips.clone(),
                None => {
                    crate::dns::lookup(
                        &options.dns,
                        &host,
                        url.port_or_known_default().unwrap_or(443),
                        &options.extra_roots,
                        &options.route,
                    )
                    .await?
                }
            };
            if ips.is_empty() {
                return Err(std::io::Error::other("No addresses found").into());
            }
            for ip in &ips {
                access.check_ip(&url, *ip).map_err(std::io::Error::other)?;
                options.route.check(*ip)?;
            }
            let addrs: Addrs = Box::new(ips.into_iter().map(|ip| std::net::SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}

pub struct CrawlRequest {
    pub url: Url,
    pub headers: HeaderMap,
    pub destination: PathBuf,
}
#[derive(Clone, Debug)]
pub struct CrawlLimits {
    pub body_bytes: u64,
    pub redirects: u8,
    pub timeout: Duration,
}
impl Default for CrawlLimits {
    fn default() -> Self {
        Self {
            body_bytes: 256 << 20,
            redirects: 10,
            timeout: Duration::from_secs(30),
        }
    }
}
#[derive(Debug)]
pub struct CrawlResponse {
    pub final_url: Url,
    pub aliases: Vec<Url>,
    pub status: StatusCode,
    pub mime: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub retry_after: Option<Duration>,
    pub bytes: u64,
    pub body: Option<PathBuf>,
    pub length: Option<u64>,
    pub sample: Vec<u8>,
}

struct PartialFile {
    path: PathBuf,
    keep: bool,
}
impl Drop for PartialFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub async fn fetch_crawl(
    request: CrawlRequest,
    options: Options,
    limits: CrawlLimits,
    budget: Arc<BodyBudget>,
    access: Arc<dyn CrawlAccess>,
    cancel: CancellationToken,
) -> Result<CrawlResponse, CrawlFetchError> {
    let worker = Box::pin(fetch_inner(
        request,
        options,
        limits,
        budget,
        access,
        FetchMode::Full,
    ));
    tokio::select! { biased;
        _ = cancel.cancelled() => Err(CrawlFetchError::Cancelled),
        result = worker => result,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FetchMode {
    Full,
    Page,
    Head,
    Sample,
}

async fn fetch_inner(
    request: CrawlRequest,
    options: Options,
    limits: CrawlLimits,
    budget: Arc<BodyBudget>,
    access: Arc<dyn CrawlAccess>,
    mode: FetchMode,
) -> Result<CrawlResponse, CrawlFetchError> {
    let mut url = request.url;
    url.set_fragment(None);
    let mut headers = request.headers;
    let mut aliases = Vec::new();
    let mut total_bytes = 0u64;
    let limiter = Arc::new(Limiter::new(options.speed_limit));
    loop {
        access.check_url(&url).map_err(CrawlFetchError::Access)?;
        // A proxy may resolve the destination itself, defeating local address checks. Never
        // silently bypass the configured proxy, nor claim its remote DNS has been checked.
        let direct = match options.proxy {
            Proxy::None => true,
            Proxy::System => {
                let (access, destination) = (access.clone(), url.clone());
                // Native proxy discovery can block. Keep it off the async runtime
                // so pause/cancel and the request deadline remain responsive.
                let lookup =
                    tokio::task::spawn_blocking(move || access.system_direct(&destination));
                match tokio::time::timeout(limits.timeout, lookup).await {
                    Ok(Ok(result)) => result.map_err(CrawlFetchError::Access)?,
                    Ok(Err(_)) => {
                        return Err(CrawlFetchError::Access(
                            "System routing lookup could not finish".into(),
                        ));
                    }
                    Err(_) => {
                        return Err(CrawlFetchError::Access(
                            "System routing lookup timed out".into(),
                        ));
                    }
                }
            }
            _ => false,
        };
        if !direct {
            return Err(CrawlFetchError::Access(
                "Site capture requires direct routing; proxy-side DNS cannot be verified".into(),
            ));
        }
        budget.pace(&url).await;
        if aliases.contains(&url) {
            return Err(CrawlFetchError::Access("Redirect loop".into()));
        }
        // Disable discovery only for this verified hop. Preserve the original selection
        // so a redirect gets its own route decision, including system bypass rules.
        let mut hop_options = options.clone();
        hop_options.proxy = Proxy::None;
        let builder = crate::download::crawl_client_builder(&hop_options, &url)
            .map_err(CrawlFetchError::Setup)?;
        let client = builder
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .dns_resolver(Arc::new(CheckedResolver {
                options: options.clone(),
                url: url.clone(),
                access: access.clone(),
            }))
            .user_agent("SwoopdlGrabber/0.1")
            .connect_timeout(limits.timeout)
            .read_timeout(limits.timeout)
            .build()
            .map_err(CrawlFetchError::Http)?;
        let mut request_headers = headers.clone();
        request_headers.insert(
            header::ACCEPT_ENCODING,
            header::HeaderValue::from_static("gzip"),
        );
        if mode == FetchMode::Sample {
            request_headers.insert(
                header::RANGE,
                header::HeaderValue::from_static("bytes=0-16383"),
            );
        }
        let method = if mode == FetchMode::Head {
            reqwest::Method::HEAD
        } else {
            reqwest::Method::GET
        };
        let response = client
            .request(method, url.clone())
            .headers(request_headers)
            .send()
            .await
            .map_err(CrawlFetchError::Http)?;
        let encoding = response
            .headers()
            .get(header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !matches!(encoding.as_str(), "" | "identity" | "gzip") {
            return Err(CrawlFetchError::Access(
                "Unsupported response content encoding".into(),
            ));
        }
        let status = response.status();
        let text = |name| {
            response
                .headers()
                .get(name)
                .and_then(|h| h.to_str().ok())
                .map(str::to_owned)
        };
        let mime = text(header::CONTENT_TYPE).unwrap_or_default();
        let etag = text(header::ETAG);
        let last_modified = text(header::LAST_MODIFIED);
        let location = text(header::LOCATION);
        let retry_after = text(header::RETRY_AFTER).and_then(|v| {
            v.parse::<u64>().ok().map(Duration::from_secs).or_else(|| {
                httpdate::parse_http_date(&v)
                    .ok()
                    .and_then(|d| d.duration_since(std::time::SystemTime::now()).ok())
            })
        });
        if let Some(delay) = retry_after {
            budget.defer(delay);
        }
        let length = if status == StatusCode::PARTIAL_CONTENT {
            text(header::CONTENT_RANGE)
                .and_then(|s| s.rsplit_once('/').and_then(|(_, n)| n.parse().ok()))
        } else {
            text(header::CONTENT_LENGTH).and_then(|s| s.parse().ok())
        };
        if mode != FetchMode::Full && matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
            if aliases.len() >= usize::from(limits.redirects) {
                return Err(CrawlFetchError::Access("Redirect limit reached".into()));
            }
            let mut next = url
                .join(
                    location.as_deref().ok_or_else(|| {
                        CrawlFetchError::Access("Redirect has no Location".into())
                    })?,
                )
                .map_err(|_| CrawlFetchError::Access("Invalid redirect".into()))?;
            next.set_fragment(None);
            headers = crate::download::scoped_headers(&headers, &url, &next);
            if url.origin() != next.origin() {
                headers.clear();
            }
            aliases.push(url);
            url = next;
            continue;
        }
        let normalized_mime = mime
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if mode == FetchMode::Page
            && !matches!(
                normalized_mime.as_str(),
                "text/html" | "application/xhtml+xml" | "text/css"
            )
        {
            return Err(CrawlFetchError::Access(
                "Navigation response is not HTML or CSS".into(),
            ));
        }
        if matches!(mode, FetchMode::Head | FetchMode::Sample) {
            let mut sample = Vec::new();
            if mode == FetchMode::Sample && status.is_success() {
                // Read only a bounded prefix even if Range is ignored or Content-Length lies.
                // Reserve budget before polling; never drain the rest of this response.
                let stream = response
                    .bytes_stream()
                    .map(|r| r.map_err(std::io::Error::other));
                let wire = StreamReader::new(Box::pin(stream));
                let reader: Pin<Box<dyn AsyncRead + Send>> = if encoding == "gzip" {
                    Box::pin(async_compression::tokio::bufread::GzipDecoder::new(
                        BufReader::new(wire),
                    ))
                } else {
                    Box::pin(wire)
                };
                let mut reader = reader.take(16 * 1024);
                let mut buffer = [0u8; 1024];
                while sample.len() < 16 * 1024 {
                    let mut lease = budget
                        .lease((16 * 1024 - sample.len()).min(buffer.len()) as u64)
                        .await?;
                    let mut output = ReadBuf::new(&mut buffer[..lease.amount as usize]);
                    let result = futures::future::poll_fn(|cx| {
                        let result = Pin::new(&mut reader).poll_read(cx, &mut output);
                        lease.produced = Some(output.filled().len() as u64);
                        result
                    })
                    .await;
                    let n = output.filled().len();
                    lease.commit(n as u64);
                    result.map_err(CrawlFetchError::Body)?;
                    if n == 0 {
                        break;
                    }
                    sample.extend_from_slice(&buffer[..n]);
                }
            }
            return Ok(CrawlResponse {
                final_url: url,
                aliases,
                status,
                mime,
                etag,
                last_modified,
                retry_after,
                bytes: sample.len() as u64,
                body: None,
                length,
                sample,
            });
        }
        let no_body = matches!(status, StatusCode::NOT_MODIFIED | StatusCode::NO_CONTENT);
        let expected = if no_body {
            Some(0)
        } else if encoding == "gzip" {
            None
        } else {
            response.content_length()
        };
        let save = status.is_success();
        let mut file = if save {
            Some(
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&request.destination)
                    .await?,
            )
        } else {
            None
        };
        let mut partial = file.as_ref().map(|_| PartialFile {
            path: request.destination.clone(),
            keep: false,
        });
        let (wire_limiter, global) = (limiter.clone(), options.global.clone());
        let wire = response.bytes_stream().then(move |result| {
            let (limiter, global) = (wire_limiter.clone(), global.clone());
            async move {
                if let Ok(bytes) = &result {
                    limiter.acquire(bytes.len() as u64).await;
                    if let Some(global) = global {
                        global.acquire(bytes.len() as u64).await;
                    }
                }
                result.map_err(std::io::Error::other)
            }
        });
        let wire = StreamReader::new(Box::pin(wire));
        let mut reader: Pin<Box<dyn AsyncRead + Send>> = if encoding == "gzip" && !no_body {
            let mut decoder =
                async_compression::tokio::bufread::GzipDecoder::new(BufReader::new(wire));
            decoder.multiple_members(true);
            Box::pin(decoder)
        } else {
            Box::pin(wire)
        };
        let size = copy_body(
            &mut reader,
            &mut file,
            BodyCopy {
                budget: budget.clone(),
                cap: limits.body_bytes,
                expected,
            },
        )
        .await?;
        total_bytes = total_bytes.saturating_add(size);
        if let Some(file) = &mut file {
            file.sync_all().await?;
        }
        drop(file);
        if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
            if aliases.len() >= usize::from(limits.redirects) {
                return Err(CrawlFetchError::Access("Redirect limit reached".into()));
            }
            let mut next = url
                .join(
                    location.as_deref().ok_or_else(|| {
                        CrawlFetchError::Access("Redirect has no Location".into())
                    })?,
                )
                .map_err(|_| CrawlFetchError::Access("Invalid redirect".into()))?;
            next.set_fragment(None);
            headers = crate::download::scoped_headers(&headers, &url, &next);
            if url.origin() != next.origin() {
                for name in [
                    header::COOKIE,
                    header::AUTHORIZATION,
                    header::PROXY_AUTHORIZATION,
                    header::REFERER,
                ] {
                    headers.remove(name);
                }
                headers.clear();
            }
            aliases.push(url);
            url = next;
            continue;
        }
        if let Some(partial) = &mut partial {
            partial.keep = true;
        }
        return Ok(CrawlResponse {
            final_url: url,
            aliases,
            status,
            mime,
            etag,
            last_modified,
            retry_after,
            bytes: total_bytes,
            body: save.then_some(request.destination),
            length,
            sample: Vec::new(),
        });
    }
}

/// Scan-only metadata. HEAD is followed by a bounded prefix to detect login/error HTML.
/// Both operations use the same checked routing, DNS, redirect and credential controls.
#[allow(clippy::too_many_arguments)]
pub async fn probe_crawl(
    url: Url,
    headers: HeaderMap,
    options: Options,
    limits: CrawlLimits,
    budget: Arc<BodyBudget>,
    access: Arc<dyn CrawlAccess>,
    cancel: CancellationToken,
) -> Result<CrawlResponse, CrawlFetchError> {
    let work = async {
        let request = || CrawlRequest {
            url: url.clone(),
            headers: headers.clone(),
            destination: PathBuf::new(),
        };
        let head = Box::pin(fetch_inner(
            request(),
            options.clone(),
            limits.clone(),
            budget.clone(),
            access.clone(),
            FetchMode::Head,
        ))
        .await?;
        if !head.status.is_success() && !matches!(head.status.as_u16(), 405 | 501) {
            return Ok(head);
        }
        // Start from the original URL so credentials never get reattached to a redirected origin.
        let mut sample = Box::pin(fetch_inner(
            request(),
            options,
            limits,
            budget,
            access,
            FetchMode::Sample,
        ))
        .await?;
        let prefix = String::from_utf8_lossy(&sample.sample)
            .trim_start_matches('\u{feff}')
            .trim_start()
            .to_ascii_lowercase();
        let html = [
            "<!doctype html",
            "<html",
            "<head",
            "<form",
            "<a ",
            "<h1",
            "<h2",
            "<body",
            "<div",
            "<p>",
            "<p ",
            "<table",
            "<ul",
            "<!--",
        ]
        .iter()
        .any(|tag| prefix.starts_with(tag));
        if html {
            sample.mime = "text/html".into();
        } else if matches!(
            sample
                .mime
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "text/html" | "application/xhtml+xml"
        ) && !sample.sample.is_empty()
        {
            sample.mime = "application/octet-stream".into();
        }
        Ok(sample)
    };
    tokio::select! { biased; _ = cancel.cancelled() => Err(CrawlFetchError::Cancelled), result = Box::pin(work) => result }
}

/// Read a navigation response only after checking its actual response MIME.
pub async fn fetch_crawl_page(
    request: CrawlRequest,
    options: Options,
    limits: CrawlLimits,
    budget: Arc<BodyBudget>,
    access: Arc<dyn CrawlAccess>,
    cancel: CancellationToken,
) -> Result<CrawlResponse, CrawlFetchError> {
    tokio::select! { biased;
        _ = cancel.cancelled() => Err(CrawlFetchError::Cancelled),
        result = Box::pin(fetch_inner(request, options, limits, budget, access, FetchMode::Page)) => result,
    }
}
