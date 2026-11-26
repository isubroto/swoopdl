use sdl_grabber::{
    CandidateMetadata, ProjectConfig, ProjectId, UrlKey,
    catalog::{CatalogStore, FileProject},
};

fn candidate(query: &str) -> CandidateMetadata {
    let url: url::Url = format!("https://example.test/series/episode.mkv?token={query}")
        .parse()
        .unwrap();
    CandidateMetadata {
        url: url.clone(),
        final_url: url,
        name: "episode.mkv".into(),
        mime: Some("video/x-matroska".into()),
        size: Some(16),
        source_page: "https://example.test/series/".parse().unwrap(),
    }
}
fn project() -> FileProject {
    FileProject::new(ProjectConfig::new(
        "Files",
        "https://example.test/series/".parse().unwrap(),
        "output".into(),
    ))
}
#[test]
fn rescan_keeps_selection_and_exact_signed_url_identity() {
    let mut p = project();
    let a = candidate("a");
    let b = candidate("b");
    let key = UrlKey::of(&a.url);
    p.merge(a.clone());
    p.candidates.get_mut(&key).unwrap().checked = true;
    p.merge(a);
    p.merge(b);
    assert_eq!(p.candidates.len(), 2);
    assert!(p.candidates[&key].checked);
    assert_eq!(p.selected_totals(), (1, 16, 0));
}
#[test]
fn selection_and_interrupted_frontier_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let store = CatalogStore::open(dir.path(), &ProjectId::parse("files").unwrap()).unwrap();
    let mut p = project();
    let a = candidate("secret");
    let key = UrlKey::of(&a.url);
    p.merge(a);
    p.candidates.get_mut(&key).unwrap().checked = true;
    p.explore_status = sdl_grabber::catalog::ExploreStatus::Exploring;
    store.checkpoint(&p).unwrap();
    let loaded = store.load().unwrap();
    assert!(loaded.candidates[&key].checked);
    assert_eq!(loaded.frontier.len(), 1);
    assert_eq!(
        loaded.explore_status,
        sdl_grabber::catalog::ExploreStatus::Stopped
    );
    assert!(!loaded.auto_download);
    let mut report = Vec::new();
    loaded.write_report(&mut report).unwrap();
    assert!(!String::from_utf8(report).unwrap().contains("secret"));
}
#[test]
fn newer_catalog_never_falls_back_to_stale_backup() {
    let dir = tempfile::tempdir().unwrap();
    let store = CatalogStore::open(dir.path(), &ProjectId::parse("files").unwrap()).unwrap();
    let p = project();
    store.checkpoint(&p).unwrap();
    store.checkpoint(&p).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.path()).unwrap()).unwrap();
    value["version"] = 999.into();
    std::fs::write(store.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(store.load().unwrap_err().to_string().contains("newer"));
}
#[test]
fn corrupt_catalog_recovers_selection_from_backup() {
    let dir = tempfile::tempdir().unwrap();
    let store = CatalogStore::open(dir.path(), &ProjectId::parse("files").unwrap()).unwrap();
    let mut p = project();
    let a = candidate("a");
    let key = UrlKey::of(&a.url);
    p.merge(a);
    p.candidates.get_mut(&key).unwrap().checked = true;
    store.checkpoint(&p).unwrap();
    store.checkpoint(&p).unwrap();
    std::fs::write(store.path(), b"interrupted").unwrap();
    assert!(store.load().unwrap().candidates[&key].checked);
}

#[test]
fn newer_format_with_different_fields_never_uses_stale_state() {
    let dir = tempfile::tempdir().unwrap();
    let store = CatalogStore::open(dir.path(), &ProjectId::parse("files").unwrap()).unwrap();
    let p = project();
    store.checkpoint(&p).unwrap();
    store.checkpoint(&p).unwrap();
    std::fs::write(store.path(), br#"{"version":999,"future_payload":{}}"#).unwrap();
    assert!(store.load().unwrap_err().to_string().contains("newer"));
}

#[test]
fn unknown_selected_sizes_do_not_become_zero_byte_files() {
    let mut p = project();
    let mut a = candidate("unknown");
    a.size = None;
    p.merge(a);
    p.check_all(true);
    assert_eq!(p.selected_totals(), (1, 0, 1));
}

#[test]
fn edited_filters_apply_to_undispatched_selection_without_retargeting_jobs() {
    let mut p = project();
    let a = candidate("unqueued");
    let b = candidate("queued");
    let akey = UrlKey::of(&a.url);
    let bkey = UrlKey::of(&b.url);
    p.merge(a);
    p.merge(b);
    p.check_all(true);
    p.candidates.get_mut(&bkey).unwrap().dispatch_id = Some("existing-dispatch".into());
    p.rules.exclude = vec!["*.mkv".into()];
    p.rescan();
    assert!(!p.candidates[&akey].checked);
    assert!(p.candidates[&bkey].checked);
    assert_eq!(
        p.candidates[&bkey].dispatch_id.as_deref(),
        Some("existing-dispatch")
    );
    p.check_all(true);
    assert!(!p.candidates[&akey].checked);
}
