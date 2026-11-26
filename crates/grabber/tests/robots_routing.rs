use sdl_engine::{Options, Proxy, crawl_fetch::NetworkAccess};
use sdl_grabber::{
    ProjectConfig, ProjectId, ProjectSnapshot, ProjectStatus,
    crawl::{CrawlTransport, run},
    store::ProjectStore,
};
use std::sync::Arc;
use tokio::sync::mpsc;

struct Fixture {
    seed: url::Url,
    log: Arc<std::sync::Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn start(status: u16, robots: Vec<u8>, encoding: &'static str, delay: u64) -> Self {
        Self::start_delays(status, robots, encoding, delay, 0).await
    }
    async fn start_delays(
        status: u16,
        robots: Vec<u8>,
        encoding: &'static str,
        delay: u64,
        body_delay: u64,
    ) -> Self {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let seed = format!("http://{}/ftp/The%20Flash/", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let requests = log.clone();
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = [0; 8192];
                let n = socket.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("").to_owned();
                requests.lock().unwrap().push(path.clone());
                let (status, body, mime, coding) = if path == "/robots.txt" {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    (status, robots.as_slice(), "text/plain", encoding)
                } else if path.ends_with("tiny.mp4") {
                    (200, b"tiny-video".as_slice(), "video/mp4", "")
                } else if path.ends_with("s01/") {
                    (
                        200,
                        b"<a href='tiny.mp4'>Video</a>".as_slice(),
                        "text/html",
                        "",
                    )
                } else {
                    (
                        200,
                        b"<a href='s01/'>Season</a><a href='../'>Parent</a>".as_slice(),
                        "text/html",
                        "",
                    )
                };
                let header = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nContent-Type: {mime}\r\nConnection: close\r\n{coding}\r\n",
                    body.len()
                );
                if socket.write_all(header.as_bytes()).await.is_ok() {
                    if body_delay != 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(body_delay)).await;
                    }
                    let _ = socket.write_all(body).await;
                }
            }
        });
        Self { seed, log, task }
    }
    async fn capture(&self, configure: impl FnOnce(&mut ProjectConfig)) -> ProjectSnapshot {
        let dir = tempfile::tempdir().unwrap();
        let mut config =
            ProjectConfig::new("Directory", self.seed.clone(), dir.path().join("output"));
        config.allow_private_seed = true;
        config.limits.attempts = 1;
        configure(&mut config);
        let store = ProjectStore::open(
            &dir.path().join("meta"),
            &ProjectId::parse("directory").unwrap(),
        )
        .unwrap();
        let (_commands, rx) = mpsc::channel(2);
        let (tx, _) = mpsc::channel(32);
        run(
            config.clone(),
            ProjectSnapshot::new(config),
            store,
            CrawlTransport::new(
                Default::default(),
                Arc::new(NetworkAccess::with_private_origin(
                    self.seed.origin().ascii_serialization(),
                )),
            ),
            rx,
            tx,
        )
        .await
        .unwrap()
    }
}
const ALLOW: &str = "# https://www.robotstxt.org/robotstxt.html\nUser-agent: *\nDisallow:\n";

#[tokio::test]
async fn empty_disallow_discovers_encoded_directory_and_tiny_video() {
    let fixture = Fixture::start(200, ALLOW.as_bytes().to_vec(), "", 0).await;
    let state = fixture.capture(|_| {}).await;
    assert_eq!(state.status, ProjectStatus::Completed);
    assert_eq!(state.resources.len(), 3);
    assert!(state.resources.values().any(|r| r.mime == "video/mp4"));
    assert_eq!(state.consumed_bytes, ALLOW.len() as u64 + 49 + 28 + 10);
    assert!(!fixture.log.lock().unwrap().iter().any(|p| p == "/ftp/"));
}

#[tokio::test]
async fn genuine_robots_disallow_never_requests_seed() {
    let fixture = Fixture::start(200, b"User-agent: *\nDisallow: /ftp/\n".to_vec(), "", 0).await;
    let state = fixture.capture(|_| {}).await;
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(*fixture.log.lock().unwrap(), ["/robots.txt"]);
    assert!(
        state
            .reports
            .iter()
            .any(|r| r.reason == "Robots disallows the destination")
    );
}

#[tokio::test]
async fn robots_http_statuses_distinguish_unavailable_from_failure() {
    for status in [404, 401, 403, 503] {
        let fixture = Fixture::start(status, Vec::new(), "", 0).await;
        let state = fixture.capture(|_| {}).await;
        if status == 404 {
            assert_eq!(state.status, ProjectStatus::Completed);
        } else {
            assert_eq!(state.status, ProjectStatus::Failed);
            assert_eq!(state.pending.len(), 1);
            assert!(
                state
                    .reports
                    .iter()
                    .any(|r| r.reason.contains(&status.to_string()))
            );
            assert!(
                !state
                    .reports
                    .iter()
                    .any(|r| r.reason.contains("Robots disallows"))
            );
            assert_eq!(*fixture.log.lock().unwrap(), ["/robots.txt"]);
        }
    }
}

#[tokio::test]
async fn robots_decode_failure_keeps_seed() {
    let fixture = Fixture::start(
        200,
        b"invalid gzip".to_vec(),
        "Content-Encoding: gzip\r\n",
        0,
    )
    .await;
    let state = fixture.capture(|_| {}).await;
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(state.pending.len(), 1);
    assert!(state.reports.iter().any(|r| r.reason.contains("decoded")));
}

#[tokio::test]
async fn robots_timeout_keeps_seed() {
    let fixture = Fixture::start(200, ALLOW.as_bytes().to_vec(), "", 2000).await;
    let state = fixture.capture(|c| c.limits.timeout_secs = 1).await;
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(state.pending.len(), 1);
    assert!(state.reports.iter().any(|r| r.reason.contains("timed out")));
}

#[tokio::test]
async fn robots_body_timeout_is_reported_as_timeout() {
    let fixture = Fixture::start_delays(200, ALLOW.as_bytes().to_vec(), "", 0, 2000).await;
    let state = fixture.capture(|c| c.limits.timeout_secs = 1).await;
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(state.pending.len(), 1);
    assert!(state.reports.iter().any(|r| r.reason.contains("timed out")));
}

#[tokio::test]
async fn robots_byte_limits_are_limited_and_keep_seed() {
    for body_limit in [true, false] {
        let fixture = Fixture::start(200, vec![b'#'; 512_010], "", 0).await;
        let state = fixture
            .capture(|c| {
                if body_limit {
                    c.limits.body_bytes = 12;
                } else {
                    c.limits.robots_bytes = 512_000;
                }
            })
            .await;
        assert_eq!(state.status, ProjectStatus::Limited);
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.consumed_bytes, if body_limit { 12 } else { 512_001 });
        assert!(
            !state
                .reports
                .iter()
                .any(|r| r.reason.contains("Robots disallows"))
        );
    }
}

#[tokio::test]
async fn pause_during_robots_lookup_keeps_durable_frontier() {
    let fixture = Fixture::start(200, ALLOW.as_bytes().to_vec(), "", 5000).await;
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new("Pause", fixture.seed.clone(), dir.path().join("output"));
    config.allow_private_seed = true;
    let store = ProjectStore::open(
        &dir.path().join("meta"),
        &ProjectId::parse("pause").unwrap(),
    )
    .unwrap();
    let (commands, rx) = mpsc::channel(2);
    let (events, _) = mpsc::channel(32);
    let worker = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store.clone(),
        CrawlTransport::new(
            Default::default(),
            Arc::new(NetworkAccess::with_private_origin(
                fixture.seed.origin().ascii_serialization(),
            )),
        ),
        rx,
        events,
    );
    let pause = async {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while fixture.log.lock().unwrap().is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        commands
            .send(sdl_grabber::crawl::CrawlCommand::Pause)
            .await
            .unwrap();
    };
    let (state, ()) = tokio::join!(worker, pause);
    let state = state.unwrap();
    assert_eq!(state.status, ProjectStatus::Paused);
    assert_eq!(state.pending.len(), 1);
    assert_eq!(store.load().unwrap().pending.len(), 1);
    assert_eq!(state.consumed_bytes, 0);
    assert!(state.reports.is_empty());
}

#[tokio::test]
async fn robots_connection_failure_is_not_a_rule_and_can_resume() {
    let fixture = Fixture::start(200, ALLOW.as_bytes().to_vec(), "", 0).await;
    fixture.task.abort();
    tokio::task::yield_now().await;
    let state = fixture.capture(|_| {}).await;
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(state.pending.len(), 1);
    assert!(
        state
            .reports
            .iter()
            .any(|r| r.reason.contains("connection failed"))
    );
    assert!(
        !state
            .reports
            .iter()
            .any(|r| r.reason.contains("Robots disallows"))
    );
}

#[tokio::test]
async fn robots_retry_after_is_observed_and_failure_is_retained() {
    let fixture = Fixture::start(503, Vec::new(), "Retry-After: 2\r\n", 0).await;
    let start = std::time::Instant::now();
    let state = fixture.capture(|c| c.limits.attempts = 2).await;
    assert_eq!(*fixture.log.lock().unwrap(), ["/robots.txt", "/robots.txt"]);
    assert!(start.elapsed() >= std::time::Duration::from_secs(3));
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(state.pending.len(), 1);
    assert!(state.reports.iter().any(|r| r.reason.contains("503")));
}

#[tokio::test]
async fn robots_file_failure_keeps_seed_and_omits_local_error_source() {
    use sdl_engine::crawl_fetch::CrawlAccess;
    struct BrokenFile {
        network: NetworkAccess,
        path: std::path::PathBuf,
    }
    impl CrawlAccess for BrokenFile {
        fn check_url(&self, url: &url::Url) -> Result<(), String> {
            self.network.check_url(url)?;
            std::fs::create_dir_all(&self.path).map_err(|_| "Fixture setup failed".into())
        }
        fn check_ip(&self, url: &url::Url, ip: std::net::IpAddr) -> Result<(), String> {
            self.network.check_ip(url, ip)
        }
    }
    let fixture = Fixture::start(200, ALLOW.as_bytes().to_vec(), "", 0).await;
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new(
        "File failure",
        fixture.seed.clone(),
        dir.path().join("private-file-location"),
    );
    config.allow_private_seed = true;
    config.limits.attempts = 1;
    let access = Arc::new(BrokenFile {
        network: NetworkAccess::with_private_origin(fixture.seed.origin().ascii_serialization()),
        path: config.destination.join("temporary/request.body"),
    });
    let store =
        ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("file").unwrap()).unwrap();
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let state = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store,
        CrawlTransport::new(Default::default(), access),
        rx,
        tx,
    )
    .await
    .unwrap();
    assert_eq!(state.status, ProjectStatus::Failed);
    assert_eq!(state.pending.len(), 1);
    assert!(state.reports.iter().any(|r| r.reason.contains("file")));
    assert!(!state.reports.iter().any(
        |r| r.reason.contains("private-file-location") || r.reason.contains("Robots disallows")
    ));
}

#[tokio::test]
async fn robots_routing_failure_is_actionable_and_seed_can_retry() {
    use sdl_engine::crawl_fetch::CrawlAccess;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct CountedAccess(NetworkAccess, Arc<AtomicUsize>);
    impl CrawlAccess for CountedAccess {
        fn check_url(&self, url: &url::Url) -> Result<(), String> {
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0.check_url(url)
        }
        fn check_ip(&self, url: &url::Url, ip: std::net::IpAddr) -> Result<(), String> {
            self.0.check_ip(url, ip)
        }
    }
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new(
        "Retry",
        server
            .url("/capture/index.html?token=private-query")
            .parse()
            .unwrap(),
        dir.path().join("output"),
    );
    config.allow_private_seed = true;
    config.limits.interval_ms = 1000;
    config.limits.attempts = 3;
    config.limits.urls = 20;
    let store = ProjectStore::open(
        &dir.path().join("meta"),
        &ProjectId::parse("retry").unwrap(),
    )
    .unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let access = Arc::new(CountedAccess(
        NetworkAccess::with_private_origin(config.seed.origin().ascii_serialization()),
        requests.clone(),
    ));
    let options = Options {
        proxy: Proxy::Manual {
            url: "http://private-user:private-password@localhost:9".into(),
            login: None,
            http: true,
            https: true,
        },
        ..Default::default()
    };
    let (_commands, rx) = mpsc::channel(2);
    let (tx, mut events) = mpsc::channel(32);
    let mut initial = ProjectSnapshot::new(config.clone());
    initial.next_run = Some(0);
    for _ in 0..config.limits.urls {
        initial.reports.push(sdl_grabber::ReportEntry {
            url: config.seed.clone(),
            reason: "Earlier skip".into(),
        });
    }
    let state = run(
        config,
        initial,
        store.clone(),
        CrawlTransport::new(options, access.clone()),
        rx,
        tx,
    )
    .await
    .unwrap();
    assert_eq!(state.status, ProjectStatus::Failed);
    assert!(
        state.next_run.is_none(),
        "failed scheduled lookup must suspend automatic retries"
    );
    assert_eq!(
        state.pending.len(),
        1,
        "failed lookup must keep the seed retryable"
    );
    assert_eq!(state.consumed_bytes, 0);
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "permanent routing failures must not retry"
    );
    assert!(server.log().is_empty());
    assert!(state.reports.iter().any(|r| r.reason.contains("routing")));
    assert!(
        !state
            .reports
            .iter()
            .any(|r| r.reason.contains("Robots disallows"))
    );
    let mut terminal = None;
    while let Ok(event) = events.try_recv() {
        terminal = Some(event);
    }
    assert!(terminal.unwrap().reason.unwrap().contains("routing"));
    let mut report = Vec::new();
    sdl_grabber::report::write_report(&state, &mut report).unwrap();
    let report = String::from_utf8(report).unwrap();
    for secret in ["private-user", "private-password", "private-query"] {
        assert!(!report.contains(secret));
    }
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let retried = run(
        state.config.clone(),
        state,
        store,
        CrawlTransport::new(Default::default(), access),
        rx,
        tx,
    )
    .await
    .unwrap();
    assert!(!retried.resources.is_empty());
}
