use sdl_grabber::{ProjectConfig, ProjectId, ProjectSnapshot, ProjectStatus, store::ProjectStore};
use url::Url;

#[test]
fn new_destination_preflight_accepts_child_but_preserves_nonempty_parent() {
    use sdl_grabber::store::validate_new_destination;
    let dir = tempfile::tempdir().unwrap();
    let existing = dir.path().join("existing.txt");
    std::fs::write(&existing, "keep me").unwrap();
    let child = dir.path().join("skk");
    validate_new_destination(&child).unwrap();
    assert!(!child.exists(), "Preflight must be read-only");
    assert!(validate_new_destination(dir.path()).is_err());
    assert!(validate_new_destination(&existing).is_err());
    assert_eq!(std::fs::read_to_string(existing).unwrap(), "keep me");
}

#[test]
fn corrupt_latest_uses_backup() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::open(dir.path(), &ProjectId::parse("docs").unwrap()).unwrap();
    let mut state = ProjectSnapshot::new(ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        dir.path().join("output"),
    ));
    store.checkpoint(&state).unwrap();
    state.status = ProjectStatus::Paused;
    store.checkpoint(&state).unwrap();
    std::fs::write(store.path(), b"broken").unwrap();
    assert_eq!(store.load().unwrap().status, ProjectStatus::Draft);
    assert!(std::fs::read(store.path()).unwrap().starts_with(b"broken"));
}

#[test]
fn restart_charges_uncertain_reservation() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::open(dir.path(), &ProjectId::parse("docs").unwrap()).unwrap();
    let mut state = ProjectSnapshot::new(ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        dir.path().join("output"),
    ));
    state.status = ProjectStatus::Running;
    state.consumed_bytes = 12;
    state.reserved_bytes = 50;
    store.checkpoint(&state).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.consumed_bytes, 62);
    assert_eq!(loaded.reserved_bytes, 0);
    assert_eq!(loaded.status, ProjectStatus::Paused);
    assert_eq!(loaded.pending.len(), 1);
}

#[test]
fn missing_destination_is_actionable() {
    use sdl_grabber::store::reconcile;
    let dir = tempfile::tempdir().unwrap();
    let state = ProjectSnapshot::new(ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        dir.path().join("removed"),
    ));
    assert!(reconcile(state, &dir.path().join("removed")).is_err());
}

#[test]
fn project_id_cannot_bypass_validation_through_json() {
    assert!(serde_json::from_str::<ProjectId>(r#""../escape""#).is_err());
}

#[test]
fn deleting_metadata_keeps_captured_files() {
    let dir = tempfile::tempdir().unwrap();
    let store =
        ProjectStore::open(&dir.path().join("meta"), &ProjectId::parse("docs").unwrap()).unwrap();
    let output = dir.path().join("output");
    store.prepare_output(&output).unwrap();
    std::fs::write(output.join("index.html"), "captured").unwrap();
    let state = ProjectSnapshot::new(ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        output.clone(),
    ));
    store.checkpoint(&state).unwrap();
    store.delete_metadata().unwrap();
    assert_eq!(
        std::fs::read_to_string(output.join("index.html")).unwrap(),
        "captured"
    );
    assert!(!store.path().exists());
}

#[test]
fn synthetic_100000_url_frontier_checkpoints_and_rejects_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let store = ProjectStore::open(dir.path(), &ProjectId::parse("large").unwrap()).unwrap();
    let mut config = ProjectConfig::new(
        "Large",
        Url::parse("https://example.test/docs/").unwrap(),
        dir.path().join("output"),
    );
    config.limits.urls = 100_000;
    let mut state = ProjectSnapshot::new(config);
    state.pending.clear();
    state.seen.clear();
    for index in 0..100_000 {
        let url = state.config.seed.join(&format!("?n={index}")).unwrap();
        state.seen.insert(sdl_grabber::UrlKey::of(&url));
        state.pending.push_back(sdl_grabber::Pending {
            url,
            kind: sdl_grabber::ResourceKind::Page,
            depth: 0,
        });
    }
    let started = std::time::Instant::now();
    store.checkpoint(&state).unwrap();
    eprintln!(
        "100000 URL checkpoint: {} ms; {} bytes",
        started.elapsed().as_millis(),
        std::fs::metadata(store.path()).unwrap().len()
    );
    let overflow = state.config.seed.join("?n=100000").unwrap();
    state.seen.insert(sdl_grabber::UrlKey::of(&overflow));
    assert!(store.checkpoint(&state).is_err());
    assert_eq!(store.load().unwrap().pending.len(), 100_000);
}
