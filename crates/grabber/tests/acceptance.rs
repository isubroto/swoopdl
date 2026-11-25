use sdl_engine::crawl_fetch::NetworkAccess;
use sdl_grabber::{
    ProjectConfig, ProjectId, ProjectSnapshot, ProjectStatus,
    crawl::{CrawlTransport, run},
    store::ProjectStore,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use url::Url;

#[tokio::test]
async fn crawl_loop_is_bounded_and_robots_disallow_never_requested() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new(
        "Docs",
        Url::parse(&server.url("/capture/index.html")).unwrap(),
        dir.path().join("output"),
    );
    config.allow_private_seed = true;
    config.limits.urls = 20;
    let store =
        ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("docs").unwrap()).unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _events) = mpsc::channel(32);
    let state = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store.clone(),
        transport,
        rx,
        tx,
    )
    .await
    .unwrap();
    assert_eq!(state.status, ProjectStatus::Completed);
    assert!(state.resources.len() >= 6);
    assert!(!server.log().iter().any(|e| e.path == "/capture/private"));
    assert!(state.reports.iter().any(|e| e.reason.contains("404")));
    assert!(store.load().unwrap().generation > 0);
    assert!(state.config.destination.join("index.html").exists());
    let previous = std::fs::read(state.config.destination.join("index.html")).unwrap();
    let mut candidate = state.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    cancel.cancel();
    let error =
        sdl_grabber::rewrite::publish_generation(&store, &mut candidate, &cancel).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(
        std::fs::read(state.config.destination.join("index.html")).unwrap(),
        previous
    );
}

#[tokio::test]
async fn pause_ack_follows_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        dir.path().join("output"),
    );
    let store =
        ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("docs").unwrap()).unwrap();
    let (tx, rx) = mpsc::channel(2);
    tx.send(sdl_grabber::crawl::CrawlCommand::Pause)
        .await
        .unwrap();
    let (events, _) = mpsc::channel(8);
    let state = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store.clone(),
        CrawlTransport::new(Default::default(), Arc::new(NetworkAccess::public())),
        rx,
        events,
    )
    .await
    .unwrap();
    assert_eq!(state.status, ProjectStatus::Paused);
    assert_eq!(store.load().unwrap().status, ProjectStatus::Paused);
    assert_eq!(state.pending.len(), 1);
}

#[tokio::test]
async fn sync_304_uses_verified_body_and_missing_body_refetches() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new(
        "Docs",
        Url::parse(&server.url("/capture/index.html")).unwrap(),
        dir.path().join("output"),
    );
    config.allow_private_seed = true;
    let store =
        ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("docs").unwrap()).unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let first = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store.clone(),
        transport.clone(),
        rx,
        tx,
    )
    .await
    .unwrap();
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let second = sdl_grabber::sync::synchronize(store.clone(), transport.clone(), rx, tx)
        .await
        .unwrap();
    assert_eq!(second.resources.len(), first.resources.len());
    assert!(second.generation > first.generation);
    assert!(server.log().iter().any(|e| e.status == 304));
    let hash = second
        .resources
        .values()
        .find(|r| r.url.path().ends_with("index.html"))
        .unwrap()
        .raw_hash
        .clone();
    std::fs::remove_file(store.raw_path(&second.config.destination, &hash).unwrap()).unwrap();
    server.clear_log();
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let third = sdl_grabber::sync::synchronize(store.clone(), transport, rx, tx)
        .await
        .unwrap();
    assert!(third.generation > second.generation);
    assert!(
        server
            .log()
            .iter()
            .any(|e| e.path.ends_with("index.html") && e.status == 200)
    );
}

#[tokio::test]
async fn transient_status_retries_three_times_and_pause_interrupts_backoff() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig::new(
        "Retry",
        Url::parse(&server.url("/gen/1K/body?fail_first=100")).unwrap(),
        dir.path().join("output"),
    );
    let store = ProjectStore::open(
        &dir.path().join("meta"),
        &ProjectId::parse("retry").unwrap(),
    )
    .unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let task_store = store.clone();
    let task = tokio::spawn(run(
        config.clone(),
        ProjectSnapshot::new(config),
        task_store,
        transport,
        rx,
        tx,
    ));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if server
                .log()
                .iter()
                .any(|l| l.path.starts_with("/gen/") && l.status == 503)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    commands
        .send(sdl_grabber::crawl::CrawlCommand::Pause)
        .await
        .unwrap();
    let state = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(state.status, ProjectStatus::Paused);
    assert!(!state.pending.is_empty());
    assert_eq!(store.load().unwrap().status, ProjectStatus::Paused);
    assert_eq!(
        server
            .log()
            .iter()
            .filter(|l| l.path.starts_with("/gen/"))
            .count(),
        1
    );
}

#[tokio::test]
async fn retry_after_survives_pause_and_resume() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig::new(
        "Delay",
        Url::parse(&server.url("/capture/retry-after")).unwrap(),
        dir.path().join("output"),
    );
    let store = ProjectStore::open(
        &dir.path().join("meta"),
        &ProjectId::parse("delay").unwrap(),
    )
    .unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let task = tokio::spawn(run(
        config.clone(),
        ProjectSnapshot::new(config.clone()),
        store.clone(),
        transport.clone(),
        rx,
        tx,
    ));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !server.log().iter().any(|l| l.path.ends_with("retry-after")) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    commands
        .send(sdl_grabber::crawl::CrawlCommand::Pause)
        .await
        .unwrap();
    task.await.unwrap().unwrap();
    let count = server.log().len();
    let (commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let task = tokio::spawn(run(config, store.load().unwrap(), store, transport, rx, tx));
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    commands
        .send(sdl_grabber::crawl::CrawlCommand::Pause)
        .await
        .unwrap();
    task.await.unwrap().unwrap();
    assert_eq!(
        server.log().len(),
        count,
        "A paused retry deadline must not cause new requests on resume"
    );
}

#[tokio::test]
async fn mime_whitespace_and_case_cannot_bypass_offline_sanitization() {
    for name in ["padded-mime", "uppercase-mime"] {
        let server = sdl_labserver::LabServer::start(Default::default())
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig::new(
            "MIME",
            Url::parse(&server.url(&format!("/capture/{name}"))).unwrap(),
            dir.path().join("output"),
        );
        let store =
            ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("mime").unwrap())
                .unwrap();
        let transport = CrawlTransport::new(
            Default::default(),
            Arc::new(NetworkAccess::with_private_origin(
                config.seed.origin().ascii_serialization(),
            )),
        );
        let (_commands, rx) = mpsc::channel(2);
        let (tx, _) = mpsc::channel(32);
        let state = run(
            config.clone(),
            ProjectSnapshot::new(config),
            store,
            transport,
            rx,
            tx,
        )
        .await
        .unwrap();
        let map = sdl_grabber::rewrite::resource_map(&state).unwrap();
        let local = map
            .get(&sdl_grabber::UrlKey::of(&state.config.seed))
            .unwrap();
        assert_eq!(local.extension().unwrap(), "html");
        let file = state
            .config
            .destination
            .join("generations")
            .join(format!("{:020}", state.generation))
            .join(local);
        let html = std::fs::read_to_string(file).unwrap();
        assert!(!html.contains("<script"));
        assert!(html.contains("Content-Security-Policy"));
    }
}

#[tokio::test]
async fn capture_drains_admitted_urls_after_url_limit() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new(
        "Bounded",
        Url::parse(&server.url("/capture/index.html")).unwrap(),
        dir.path().join("output"),
    );
    config.limits.urls = 3;
    let store = ProjectStore::open(
        &dir.path().join("meta"),
        &ProjectId::parse("bounded").unwrap(),
    )
    .unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let state = run(
        config.clone(),
        ProjectSnapshot::new(config),
        store,
        transport,
        rx,
        tx,
    )
    .await
    .unwrap();
    assert_eq!(state.status, ProjectStatus::Limited);
    assert_eq!(state.seen.len(), 3);
    assert_eq!(state.resources.len(), 3);
    assert!(state.pending.is_empty());
}

#[tokio::test]
async fn capture_publication_failure_never_persists_completed_and_can_resume_offline() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig::new(
        "Publish",
        Url::parse(&server.url("/capture/padded-mime")).unwrap(),
        dir.path().join("output"),
    );
    let store = ProjectStore::open(
        &dir.path().join("meta"),
        &ProjectId::parse("publish").unwrap(),
    )
    .unwrap();
    store.prepare_output(&config.destination).unwrap();
    std::fs::create_dir(config.destination.join("index.html")).unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    assert!(
        run(
            config.clone(),
            ProjectSnapshot::new(config.clone()),
            store.clone(),
            transport.clone(),
            rx,
            tx
        )
        .await
        .is_err()
    );
    let state = store.load().unwrap();
    assert_ne!(state.status, ProjectStatus::Completed);
    std::fs::remove_dir(config.destination.join("index.html")).unwrap();
    server.clear_log();
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let state = run(config, state, store, transport, rx, tx).await.unwrap();
    assert_eq!(state.status, ProjectStatus::Completed);
    assert!(state.config.destination.join("index.html").is_file());
    assert!(server.log().is_empty());
}

#[tokio::test]
async fn paused_resource_limit_cannot_resume_as_completed() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut config = ProjectConfig::new(
        "Resource cap",
        Url::parse(&server.url("/capture/index.html")).unwrap(),
        dir.path().join("output"),
    );
    config.limits.resource_bytes = 1024;
    let store =
        ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("cap").unwrap()).unwrap();
    let transport = CrawlTransport::new(
        Default::default(),
        Arc::new(NetworkAccess::with_private_origin(
            config.seed.origin().ascii_serialization(),
        )),
    );
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let mut state = run(
        config.clone(),
        ProjectSnapshot::new(config.clone()),
        store.clone(),
        transport.clone(),
        rx,
        tx,
    )
    .await
    .unwrap();
    assert_eq!(state.status, ProjectStatus::Limited);
    assert!(state.pending.is_empty());
    state.status = ProjectStatus::Paused;
    store.checkpoint(&state).unwrap();
    server.clear_log();
    let (_commands, rx) = mpsc::channel(2);
    let (tx, _) = mpsc::channel(32);
    let state = run(config, store.load().unwrap(), store, transport, rx, tx)
        .await
        .unwrap();
    assert_eq!(state.status, ProjectStatus::Limited);
    assert!(server.log().is_empty());
}
