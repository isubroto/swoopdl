//! Conditional recapture retains originals and publishes only a finished generation.
use crate::{
    crawl::{CrawlCommand, CrawlError, CrawlEvent, CrawlTransport},
    store::ProjectStore,
    *,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
pub fn due_run(state: &ProjectSnapshot, now: SystemTime) -> bool {
    matches!(
        state.status,
        ProjectStatus::Completed | ProjectStatus::Limited | ProjectStatus::Failed
    ) && state.next_run.is_some_and(|due| {
        now.duration_since(UNIX_EPOCH)
            .is_ok_and(|n| n.as_secs() >= due)
    })
}
pub async fn synchronize(
    store: ProjectStore,
    transport: CrawlTransport,
    commands: mpsc::Receiver<CrawlCommand>,
    events: mpsc::Sender<CrawlEvent>,
) -> Result<ProjectSnapshot, CrawlError> {
    let mut state = store.load()?;
    if !state.config.destination.is_dir() {
        return Err(std::io::Error::other("Capture destination is missing"));
    }
    // Synchronize explicitly starts a new run. Start / Resume calls run directly,
    // retaining an interrupted run's existing accounting and frontier.
    {
        state.consumed_bytes = 0;
        state.reserved_bytes = 0;
        state.limit_reached = false;
        state.reports.clear();
        state.seen = [UrlKey::of(&state.config.seed)].into();
        state.pending = [Pending {
            url: state.config.seed.clone(),
            kind: ResourceKind::Page,
            depth: 0,
        }]
        .into();
    }
    state.next_run = state.sync_interval_secs.and_then(|seconds| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs()
            .checked_add(seconds)
    });
    state.status = ProjectStatus::Running;
    store.checkpoint(&state)?;
    crate::crawl::run(
        state.config.clone(),
        state,
        store,
        transport,
        commands,
        events,
    )
    .await
}
