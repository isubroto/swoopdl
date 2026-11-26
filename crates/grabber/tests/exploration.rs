use sdl_grabber::{ProjectConfig, ResourceKind, discover::discover, policy::save_allowed};

#[test]
fn file_links_do_not_consume_page_depth() {
    let url = "https://example.test/series/season/".parse().unwrap();
    let links = discover(b"<a href='episode.mkv'>Episode</a>", "text/html", &url).unwrap();
    assert_eq!(links[0].kind, ResourceKind::Asset);
    let mut config = ProjectConfig::new("Files", url, "output".into());
    config.limits.depth = 1;
    assert_eq!(
        sdl_grabber::evaluate(&config, &links[0].url, links[0].kind, 2),
        sdl_grabber::Decision::Allow
    );
}

#[test]
fn file_patterns_apply_to_anchor_downloads() {
    let url = "https://example.test/series/".parse().unwrap();
    let mut config = ProjectConfig::new("Files", url, "output".into());
    config.extensions.insert("mp4".into());
    let links = discover(
        b"<a href='episode.mkv'>Episode</a>",
        "text/html",
        &config.seed,
    )
    .unwrap();
    assert!(!save_allowed(
        &config,
        &links[0].url,
        links[0].kind,
        "video/x-matroska"
    ));
}

#[tokio::test]
async fn depth_one_directory_scan_lists_videos_without_downloading_them() {
    use sdl_engine::crawl_fetch::NetworkAccess;
    use sdl_grabber::{
        crawl::CrawlTransport,
        explore::{ExploreEvent, explore},
    };
    use std::sync::Arc;
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let seed = server.url("/capture/flash/").parse().unwrap();
    let mut config = ProjectConfig::new("Files", seed, dir.path().join("unused"));
    config.limits.depth = 1;
    config.extensions = ["mkv".into(), "mp4".into()].into();
    config.allow_private_seed = true;
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let events = explore(&config, transport, Default::default())
        .await
        .unwrap();
    let candidates: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let ExploreEvent::CandidateFound(c) = e {
                Some(c)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(candidates.len(), 2, "{events:?}");
    assert!(
        candidates
            .iter()
            .all(|c| c.source_page.path().ends_with("season/"))
    );
    assert!(candidates.iter().any(|c| c.name == "episode.mkv"));
    assert!(!server.log().iter().any(|e| e.path == "/capture/parent"));
    assert!(
        !server
            .log()
            .iter()
            .any(|e| (e.path.ends_with(".mkv") || e.path.ends_with(".mp4"))
                && e.method == "GET"
                && e.range.is_none())
    );
    assert!(!config.destination.exists());
}

#[tokio::test]
async fn html_disguised_as_video_is_not_a_video() {
    use sdl_engine::crawl_fetch::{BodyBudget, CrawlLimits, NetworkAccess, probe_crawl};
    use std::sync::Arc;
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let url: url::Url = server
        .url("/capture/flash/season/login.mkv")
        .parse()
        .unwrap();
    let result = probe_crawl(
        url.clone(),
        Default::default(),
        Default::default(),
        CrawlLimits::default(),
        Arc::new(BodyBudget::new(65536, 0)),
        Arc::new(NetworkAccess::with_private_origin(
            url.origin().ascii_serialization(),
        )),
        Default::default(),
    )
    .await
    .unwrap();
    assert_eq!(result.mime, "text/html");
    assert!(result.sample.len() <= 16 * 1024);
}

#[tokio::test]
async fn ignored_range_and_unsupported_head_never_drain_media() {
    use sdl_engine::crawl_fetch::{BodyBudget, CrawlLimits, NetworkAccess, probe_crawl};
    use std::sync::Arc;
    for path in ["/capture/probe-no-head.mkv", "/capture/probe-large.mkv"] {
        let server = sdl_labserver::LabServer::start(Default::default())
            .await
            .unwrap();
        let url: url::Url = server.url(path).parse().unwrap();
        let budget = Arc::new(BodyBudget::new(32768, 0));
        let result = probe_crawl(
            url.clone(),
            Default::default(),
            Default::default(),
            CrawlLimits::default(),
            budget.clone(),
            Arc::new(NetworkAccess::with_private_origin(
                url.origin().ascii_serialization(),
            )),
            Default::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            result.sample.len(),
            16384,
            "{path}: {result:?}; requests: {:?}",
            server.log()
        );
        assert_eq!(budget.consumed(), 16384);
        assert_eq!(result.length, None);
        assert!(
            server
                .log()
                .iter()
                .filter(|e| e.method == "GET")
                .all(|e| e.range.as_deref() == Some("bytes=0-16383"))
        );
    }
}

#[tokio::test]
async fn cancelling_robots_keeps_the_current_frontier_for_resume() {
    use std::sync::Arc;
    use tokio::io::AsyncReadExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let seed: url::Url = format!("http://{}/series/", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    let received = Arc::new(tokio::sync::Notify::new());
    let notify = received.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut b = [0; 4096];
        let _ = socket.read(&mut b).await;
        notify.notify_one();
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    });
    let dir = tempfile::tempdir().unwrap();
    let mut p = sdl_grabber::catalog::FileProject::new(ProjectConfig::new(
        "Files",
        seed.clone(),
        dir.path().join("output"),
    ));
    p.config.allow_private_seed = true;
    let cancel = tokio_util::sync::CancellationToken::new();
    let stop = cancel.clone();
    let task = tokio::spawn(async move {
        let transport = sdl_grabber::crawl::CrawlTransport::new(
            Default::default(),
            Arc::new(sdl_engine::crawl_fetch::NetworkAccess::with_private_origin(
                seed.origin().ascii_serialization(),
            )),
        );
        sdl_grabber::explore::explore_catalog(&mut p, None, transport, cancel, None)
            .await
            .unwrap();
        p
    });
    received.notified().await;
    stop.cancel();
    let state = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    server.abort();
    assert_eq!(state.frontier.len(), 1);
    assert!(
        !state
            .seen
            .contains(&sdl_grabber::UrlKey::of(&state.config.seed))
    );
    assert_eq!(
        state.explore_status,
        sdl_grabber::catalog::ExploreStatus::Stopped
    );
}

#[tokio::test]
async fn failed_robots_lookup_is_not_reported_as_a_real_exclusion() {
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let seed: url::Url = format!("http://{}/series/", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    let server = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut b = [0; 4096];
            let _ = socket.read(&mut b).await;
            let _ = socket
                .write_all(
                    b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mut p = sdl_grabber::catalog::FileProject::new(ProjectConfig::new(
        "Files",
        seed.clone(),
        dir.path().join("output"),
    ));
    p.config.allow_private_seed = true;
    let transport = sdl_grabber::crawl::CrawlTransport::new(
        Default::default(),
        Arc::new(sdl_engine::crawl_fetch::NetworkAccess::with_private_origin(
            seed.origin().ascii_serialization(),
        )),
    );
    sdl_grabber::explore::explore_catalog(&mut p, None, transport, Default::default(), None)
        .await
        .unwrap();
    server.abort();
    assert_eq!(
        p.explore_status,
        sdl_grabber::catalog::ExploreStatus::Failed
    );
    assert_eq!(p.frontier.len(), 1);
    assert!(
        p.reports
            .iter()
            .any(|r| r.reason.contains("Robots lookup failed: HTTP 503"))
    );
}
