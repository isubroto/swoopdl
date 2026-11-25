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
