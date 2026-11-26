use sdl_grabber::{
    CandidateMetadata, ProjectConfig,
    catalog::{CollisionPolicy, SaveLayout},
    destinations::reserve,
};
fn metadata(path: &str) -> CandidateMetadata {
    let url: url::Url = format!("https://example.test/series/{path}")
        .parse()
        .unwrap();
    CandidateMetadata {
        name: url.path_segments().unwrap().next_back().unwrap().into(),
        url: url.clone(),
        final_url: url,
        mime: Some("video/x-matroska".into()),
        size: Some(16),
        source_page: "https://example.test/series/season/".parse().unwrap(),
    }
}
#[test]
fn original_names_and_season_folders_preserve_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("unrelated.txt"), b"keep").unwrap();
    let config = ProjectConfig::new(
        "Files",
        "https://example.test/series/".parse().unwrap(),
        dir.path().to_owned(),
    );
    let a = reserve(
        &config,
        &metadata("season/episode.mkv"),
        SaveLayout::OriginalSubfolders,
        CollisionPolicy::Number,
        &Default::default(),
    )
    .unwrap()
    .unwrap();
    assert!(a.ends_with("season/episode.mkv"));
    std::fs::create_dir_all(a.parent().unwrap()).unwrap();
    std::fs::write(&a, b"existing").unwrap();
    let b = reserve(
        &config,
        &metadata("season/episode.mkv"),
        SaveLayout::OriginalSubfolders,
        CollisionPolicy::Number,
        &Default::default(),
    )
    .unwrap()
    .unwrap();
    assert_ne!(a, b);
    assert_eq!(std::fs::read(a).unwrap(), b"existing");
    assert_eq!(
        std::fs::read(dir.path().join("unrelated.txt")).unwrap(),
        b"keep"
    );
}
#[test]
fn encoded_separators_traversal_and_windows_devices_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let config = ProjectConfig::new(
        "Files",
        "https://example.test/series/".parse().unwrap(),
        dir.path().to_owned(),
    );
    for path in [
        "season/%2fescape.mkv",
        "season/%5cescape.mkv",
        "season/CON.mkv",
        "season/COM%C2%B9.mkv",
        "season/file.mkv%3Astream",
        "season/%252fescape.mkv",
    ] {
        assert!(
            reserve(
                &config,
                &metadata(path),
                SaveLayout::OriginalSubfolders,
                CollisionPolicy::Number,
                &Default::default()
            )
            .is_err(),
            "{path}"
        );
    }
}
