use sdl_engine::crawl_fetch::{CaptureGuard, NetworkAccess};
use sdl_engine::{Destination, Download, Options, Request};
use std::sync::Arc;

#[tokio::test]
async fn normal_engine_guard_refuses_private_destination_before_connection() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = Options {
        capture_guard: Some(CaptureGuard(Arc::new(NetworkAccess::public()))),
        ..Default::default()
    };
    let dl = Download::start(
        Request {
            url: server.url("/gen/65536").parse().unwrap(),
            headers: Default::default(),
            dest: Destination::File(dir.path().join("video.mkv")),
        },
        options,
    );
    assert!(dl.wait().await.is_err());
    assert!(server.log().is_empty());
}

#[tokio::test]
async fn every_guarded_redirect_is_checked_before_connection() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url: url::Url = server
        .url(&format!(
            "/redirect?to=http://localhost:{}/gen/65536",
            server.addr().port()
        ))
        .parse()
        .unwrap();
    let origin = url.origin().ascii_serialization();
    let options = Options {
        capture_guard: Some(CaptureGuard(Arc::new(
            NetworkAccess::with_private_origin(origin.clone()).restrict([origin].into()),
        ))),
        ..Default::default()
    };
    let dl = Download::start(
        Request {
            url,
            headers: Default::default(),
            dest: Destination::File(dir.path().join("video.mkv")),
        },
        options,
    );
    assert!(dl.wait().await.is_err());
    assert!(!server.log().iter().any(|e| e.path == "/gen/65536"));
}

#[tokio::test]
async fn guarded_engine_checks_dns_answers_before_any_connection() {
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://public.test:{}/gen/65536", server.addr().port())
        .parse()
        .unwrap();
    let options = Options {
        hosts: vec![("public.test".into(), vec!["127.0.0.1".parse().unwrap()])],
        capture_guard: Some(CaptureGuard(Arc::new(NetworkAccess::public()))),
        ..Default::default()
    };
    let dl = Download::start(
        Request {
            url,
            headers: Default::default(),
            dest: Destination::File(dir.path().join("episode.mkv")),
        },
        options,
    );
    assert!(dl.wait().await.is_err());
    assert!(server.log().is_empty());
}

#[tokio::test]
async fn guarded_cross_origin_redirect_strips_session_and_referrer() {
    use sdl_engine::crawl_fetch::CrawlAccess;
    use std::{collections::BTreeSet, net::IpAddr};
    struct LocalOrigins(BTreeSet<String>);
    impl CrawlAccess for LocalOrigins {
        fn check_url(&self, url: &url::Url) -> Result<(), String> {
            if self.0.contains(&url.origin().ascii_serialization()) {
                Ok(())
            } else {
                Err("Outside test origins".into())
            }
        }
        fn check_ip(&self, _: &url::Url, ip: IpAddr) -> Result<(), String> {
            if ip.is_loopback() {
                Ok(())
            } else {
                Err("Outside test loopback".into())
            }
        }
    }
    let a = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let b = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = b.url("/gen/4096");
    let url: url::Url = a.url(&format!("/redirect?to={target}")).parse().unwrap();
    let origins = [
        url.origin().ascii_serialization(),
        url::Url::parse(&target)
            .unwrap()
            .origin()
            .ascii_serialization(),
    ]
    .into();
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in [
        ("authorization", "Bearer secret"),
        ("cookie", "session=secret"),
        ("referer", "https://source.test/private"),
    ] {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
    }
    let dl = Download::start(
        Request {
            url,
            headers,
            dest: Destination::File(dir.path().join("episode.mkv")),
        },
        Options {
            capture_guard: Some(CaptureGuard(Arc::new(LocalOrigins(origins)))),
            ..Default::default()
        },
    );
    dl.wait().await.unwrap();
    let entries = b.log();
    assert!(!entries.is_empty());
    assert!(
        entries
            .iter()
            .all(|e| e.cookie.is_none() && e.authorization.is_none() && e.referer.is_none())
    );
}

#[tokio::test]
async fn guarded_redirects_keep_the_project_redirect_ceiling() {
    use sdl_grabber::{
        ProjectConfig, ProjectId, UrlKey,
        catalog::FileProject,
        jobs::{CapturePolicy, SiteDownloadRequest},
    };
    let server = sdl_labserver::LabServer::start(Default::default())
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut inner: url::Url = server.url("/redirect").parse().unwrap();
    inner
        .query_pairs_mut()
        .append_pair("to", &server.url("/gen/4096"));
    let mut url: url::Url = server.url("/redirect").parse().unwrap();
    url.query_pairs_mut().append_pair("to", inner.as_str());
    let mut p = FileProject::new(ProjectConfig::new(
        "Files",
        server.url("/").parse().unwrap(),
        dir.path().join("output"),
    ));
    p.config.allow_private_seed = true;
    p.config.limits.redirects = 1;
    let project_id = ProjectId::parse("redirect-ceiling").unwrap();
    let candidate_id = UrlKey::of(&url);
    let destination = sdl_grabber::paths::checked_path(
        &p.config.destination,
        std::path::Path::new("episode.mkv"),
    )
    .unwrap();
    let request = SiteDownloadRequest {
        dispatch_id: SiteDownloadRequest::identity(&project_id, &candidate_id),
        project_id,
        candidate_id,
        scan_revision: 1,
        url: url.clone(),
        name: "episode.mkv".into(),
        destination: destination.clone(),
        expected_bytes: Some(4096),
        source_page: p.config.seed.clone(),
        policy: CapturePolicy::snapshot(&p, &Default::default()).unwrap(),
        item_id: 1,
    };
    let (mut options, guard) = request.guard(Default::default()).unwrap();
    options.capture_guard = Some(guard);
    assert!(
        Download::start(
            Request {
                url,
                headers: Default::default(),
                dest: Destination::File(destination)
            },
            options
        )
        .wait()
        .await
        .is_err()
    );
    assert!(!server.log().iter().any(|e| e.path == "/gen/4096"));
}
