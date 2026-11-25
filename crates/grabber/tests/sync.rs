use sdl_grabber::{ProjectConfig, ProjectSnapshot, ProjectStatus, sync::due_run};
use std::time::{Duration, UNIX_EPOCH};
use url::Url;

#[test]
fn manual_pause_suppresses_schedule() {
    let mut s = ProjectSnapshot::new(ProjectConfig::new(
        "Docs",
        Url::parse("https://example.test/").unwrap(),
        "output".into(),
    ));
    s.next_run = Some(10);
    s.status = ProjectStatus::Paused;
    assert!(!due_run(&s, UNIX_EPOCH + Duration::from_secs(20)));
    s.status = ProjectStatus::Completed;
    assert!(due_run(&s, UNIX_EPOCH + Duration::from_secs(20)));
    assert!(!due_run(&s, UNIX_EPOCH + Duration::from_secs(9)));
}
