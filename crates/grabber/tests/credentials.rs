use sdl_grabber::{CredentialRef, ProjectConfig, ProjectSnapshot};
use url::Url;

#[test]
fn serialized_project_contains_only_protected_reference() {
    let config = ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        "output".into(),
    );
    let mut state = ProjectSnapshot::new(config);
    state.credential = Some(CredentialRef {
        identifier: "site-docs".into(),
        origin: "https://example.test".into(),
    });
    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("Bearer fixture-secret"));
    assert!(json.contains("site-docs"));
}
