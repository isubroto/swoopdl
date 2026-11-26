use sdl_engine::crawl_fetch::{BodyBudget, CrawlAccess, NetworkAccess};
use std::{net::IpAddr, sync::Arc};
use url::Url;

#[tokio::test]
async fn redirect_checked_before_request() {
    use sdl_engine::crawl_fetch::{CrawlLimits, CrawlRequest, fetch_crawl};
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let url = Url::parse(&server.url(&format!(
        "/redirect?to=http://localhost:{}/capture/private",
        server.addr().port()
    )))
    .unwrap();
    let origin = url.origin().ascii_serialization();
    let dir = tempfile::tempdir().unwrap();
    let access = NetworkAccess::with_private_origin(origin.clone()).restrict([origin].into());
    let result = fetch_crawl(
        CrawlRequest {
            url,
            headers: Default::default(),
            destination: dir.path().join("body"),
        },
        Default::default(),
        CrawlLimits::default(),
        Arc::new(BodyBudget::new(1024, 0)),
        Arc::new(access),
        Default::default(),
    )
    .await;
    assert!(result.is_err());
    assert!(!server.log().iter().any(|e| e.path == "/capture/private"));
    assert!(!dir.path().join("body").exists());
}

#[tokio::test]
async fn dns_answer_is_checked_before_connection() {
    use sdl_engine::crawl_fetch::{CrawlLimits, CrawlRequest, fetch_crawl};
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let url = Url::parse(&format!(
        "http://public.test:{}/capture/index.html",
        server.addr().port()
    ))
    .unwrap();
    let options = sdl_engine::Options {
        hosts: vec![("public.test".into(), vec!["127.0.0.1".parse().unwrap()])],
        ..Default::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let result = fetch_crawl(
        CrawlRequest {
            url,
            headers: Default::default(),
            destination: dir.path().join("body"),
        },
        options,
        CrawlLimits::default(),
        Arc::new(BodyBudget::new(1024, 0)),
        Arc::new(NetworkAccess::public()),
        Default::default(),
    )
    .await;
    assert!(result.is_err());
    assert!(server.log().is_empty());
}

#[tokio::test]
async fn chunked_body_obeys_budget_and_cleans_partial_file() {
    use sdl_engine::crawl_fetch::{CrawlLimits, CrawlRequest, fetch_crawl};
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let url = Url::parse(&server.url("/capture/chunked")).unwrap();
    let access = NetworkAccess::with_private_origin(url.origin().ascii_serialization());
    let dir = tempfile::tempdir().unwrap();
    let budget = Arc::new(BodyBudget::new(1024, 0));
    let result = fetch_crawl(
        CrawlRequest {
            url,
            headers: Default::default(),
            destination: dir.path().join("body"),
        },
        Default::default(),
        CrawlLimits::default(),
        budget.clone(),
        Arc::new(access),
        Default::default(),
    )
    .await;
    assert!(result.is_err(), "{result:?}; log: {:?}", server.log());
    assert!(budget.consumed() <= 1024);
    assert!(!dir.path().join("body").exists());
}

#[tokio::test]
async fn decoded_gzip_body_obeys_budget() {
    use sdl_engine::crawl_fetch::{CrawlFetchError, CrawlLimits, CrawlRequest, fetch_crawl};
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let url = Url::parse(&server.url("/capture/gzip")).unwrap();
    let access = NetworkAccess::with_private_origin(url.origin().ascii_serialization());
    let dir = tempfile::tempdir().unwrap();
    let budget = Arc::new(BodyBudget::new(512, 0));
    let result = fetch_crawl(
        CrawlRequest {
            url,
            headers: Default::default(),
            destination: dir.path().join("body"),
        },
        Default::default(),
        CrawlLimits::default(),
        budget.clone(),
        Arc::new(access),
        Default::default(),
    )
    .await;
    assert!(matches!(result, Err(CrawlFetchError::Budget)), "{result:?}");
    assert_eq!(budget.consumed(), 512);
    assert!(!dir.path().join("body").exists());
}

#[test]
fn parallel_retries_share_budget() {
    let b = Arc::new(BodyBudget::new(10, 0));
    let threads: Vec<_> = (0..20)
        .map(|_| {
            let b = b.clone();
            std::thread::spawn(move || b.charge(1).is_ok())
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .filter(|t| t.thread().id() != std::thread::current().id())
            .map(|t| usize::from(t.join().unwrap()))
            .sum::<usize>(),
        10
    );
    assert_eq!(b.consumed(), 10);
}

#[test]
fn dns_rebinding_peer_is_rejected() {
    let u = Url::parse("https://public.test/").unwrap();
    let p = NetworkAccess::public();
    for ip in [
        "127.0.0.1",
        "169.254.169.254",
        "10.0.0.1",
        "::1",
        "::ffff:127.0.0.1",
        "fd00::1",
    ] {
        assert!(
            p.check_ip(&u, ip.parse::<IpAddr>().unwrap()).is_err(),
            "{ip}"
        );
    }
    assert!(p.check_ip(&u, "93.184.216.34".parse().unwrap()).is_ok());
    let intranet = NetworkAccess::with_private_origin("https://intranet.test".into());
    assert!(
        intranet
            .check_ip(
                &Url::parse("https://intranet.test/").unwrap(),
                "10.0.0.1".parse().unwrap()
            )
            .is_ok()
    );
    assert!(intranet.check_ip(&u, "10.0.0.1".parse().unwrap()).is_err());
}

#[tokio::test]
async fn credentials_do_not_cross_origin() {
    use sdl_engine::crawl_fetch::{CrawlLimits, CrawlRequest, fetch_crawl};
    struct Loopback;
    impl CrawlAccess for Loopback {
        fn check_url(&self, u: &Url) -> Result<(), String> {
            if u.scheme() == "http" && matches!(u.host_str(), Some("localhost" | "127.0.0.1")) {
                Ok(())
            } else {
                Err("fixture scope".into())
            }
        }
        fn check_ip(&self, _: &Url, ip: IpAddr) -> Result<(), String> {
            if ip.is_loopback() {
                Ok(())
            } else {
                Err("fixture address".into())
            }
        }
    }
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let target = format!(
        "http://localhost:{}/capture/index.html",
        server.addr().port()
    );
    let url = Url::parse(&server.url(&format!("/redirect?to={target}"))).unwrap();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        "Bearer fixture-secret".parse().unwrap(),
    );
    headers.insert(
        reqwest::header::COOKIE,
        "session=fixture-secret".parse().unwrap(),
    );
    let dir = tempfile::tempdir().unwrap();
    fetch_crawl(
        CrawlRequest {
            url,
            headers,
            destination: dir.path().join("body"),
        },
        Default::default(),
        CrawlLimits::default(),
        Arc::new(BodyBudget::new(1 << 20, 0)),
        Arc::new(Loopback),
        Default::default(),
    )
    .await
    .unwrap();
    let log = server.log();
    let target = log
        .iter()
        .find(|l| l.path == "/capture/index.html")
        .unwrap();
    assert!(target.authorization.is_none());
    assert!(target.cookie.is_none());
}
struct RoutedAccess {
    network: NetworkAccess,
    proxy_on_path: Option<&'static str>,
    fail_resolution: bool,
    decisions: std::sync::Mutex<Vec<String>>,
}

#[tokio::test]
async fn system_route_resolution_obeys_timeout_and_cancellation() {
    use sdl_engine::crawl_fetch::{CrawlFetchError, CrawlLimits, CrawlRequest, fetch_crawl};
    use std::time::{Duration, Instant};
    struct SlowRoute(NetworkAccess);
    impl CrawlAccess for SlowRoute {
        fn check_url(&self, url: &Url) -> Result<(), String> {
            self.0.check_url(url)
        }
        fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), String> {
            self.0.check_ip(url, ip)
        }
        fn system_direct(&self, _: &Url) -> Result<bool, String> {
            std::thread::sleep(Duration::from_millis(500));
            Ok(true)
        }
    }
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    for cancel_request in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let url: Url = server.url("/capture/index.html").parse().unwrap();
        let access = Arc::new(SlowRoute(NetworkAccess::with_private_origin(
            url.origin().ascii_serialization(),
        )));
        let cancel = tokio_util::sync::CancellationToken::new();
        let token = cancel.clone();
        let trigger = tokio::spawn(async move {
            if cancel_request {
                tokio::time::sleep(Duration::from_millis(10)).await;
                token.cancel();
            }
        });
        let started = Instant::now();
        let result = fetch_crawl(
            CrawlRequest {
                url,
                headers: Default::default(),
                destination: dir.path().join("body"),
            },
            sdl_engine::Options {
                proxy: sdl_engine::Proxy::System,
                ..Default::default()
            },
            CrawlLimits {
                timeout: Duration::from_millis(50),
                ..Default::default()
            },
            Arc::new(BodyBudget::new(100_000, 0)),
            access,
            cancel,
        )
        .await;
        assert!(
            started.elapsed() < Duration::from_millis(300),
            "route lookup blocked the runtime"
        );
        if cancel_request {
            assert!(matches!(result, Err(CrawlFetchError::Cancelled)));
        } else {
            assert!(
                matches!(result,Err(CrawlFetchError::Access(ref reason)) if reason.contains("timed out"))
            );
        }
        trigger.await.unwrap();
        assert!(server.log().is_empty());
        assert!(!dir.path().join("body").exists());
    }
}
impl CrawlAccess for RoutedAccess {
    fn check_url(&self, url: &Url) -> Result<(), String> {
        self.network.check_url(url)
    }
    fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), String> {
        self.network.check_ip(url, ip)
    }
    fn system_direct(&self, url: &Url) -> Result<bool, String> {
        self.decisions.lock().unwrap().push(url.path().into());
        if self.fail_resolution {
            return Err("System routing could not be resolved".into());
        }
        Ok(self.proxy_on_path.is_none_or(|path| url.path() != path))
    }
}

#[tokio::test]
async fn system_direct_is_checked_per_redirect_and_configured_proxy_is_never_bypassed() {
    use sdl_engine::crawl_fetch::{CrawlFetchError, CrawlLimits, CrawlRequest, fetch_crawl};
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    for (path, blocked, unknown, succeeds) in [
        ("/capture/index.html".to_owned(), None, false, true),
        (
            "/capture/index.html".to_owned(),
            Some("/capture/index.html"),
            false,
            false,
        ),
        ("/capture/index.html".to_owned(), None, true, false),
        (
            "/redirect?to=/capture/private".to_owned(),
            Some("/capture/private"),
            false,
            false,
        ),
    ] {
        server.clear_log();
        let url: Url = server.url(&path).parse().unwrap();
        let access = Arc::new(RoutedAccess {
            network: NetworkAccess::with_private_origin(url.origin().ascii_serialization()),
            proxy_on_path: blocked,
            fail_resolution: unknown,
            decisions: Default::default(),
        });
        let dir = tempfile::tempdir().unwrap();
        let result = fetch_crawl(
            CrawlRequest {
                url,
                headers: Default::default(),
                destination: dir.path().join("body"),
            },
            sdl_engine::Options {
                proxy: sdl_engine::Proxy::System,
                ..Default::default()
            },
            CrawlLimits::default(),
            Arc::new(BodyBudget::new(100_000, 0)),
            access.clone(),
            Default::default(),
        )
        .await;
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        if !succeeds {
            assert!(matches!(result, Err(CrawlFetchError::Access(_))));
            assert!(
                !server
                    .log()
                    .iter()
                    .any(|r| Some(r.path.as_str()) == blocked)
            );
            assert!(!dir.path().join("body").exists());
        }
        let decisions = access.decisions.lock().unwrap();
        assert_eq!(
            decisions.len(),
            if path.starts_with("/redirect") { 2 } else { 1 }
        );
    }
}
