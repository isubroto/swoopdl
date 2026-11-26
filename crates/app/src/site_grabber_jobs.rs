//! Durable guarded handoff. This store is deliberately invisible to older download-list readers.
use crate::model::{Category, Item, Status};
use sdl_grabber::{
    ProjectId,
    catalog::{CatalogStore, FileProject, TransferStatus},
    jobs::{CapturePolicy, SiteDownloadRequest},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    sync::Mutex,
};

static LOCK: Mutex<()> = Mutex::new(());
const VERSION: u32 = 1;
const MAX_METADATA: usize = 128 << 20;
#[derive(Serialize, Deserialize)]
struct Record {
    guard: SiteDownloadRequest,
    item: Item,
    exposed: bool,
    removed: bool,
}
#[derive(Serialize, Deserialize)]
struct Disk {
    version: u32,
    records: BTreeMap<String, Record>,
}
fn empty() -> Disk {
    Disk {
        version: VERSION,
        records: BTreeMap::new(),
    }
}
pub fn path() -> io::Result<PathBuf> {
    sdl_ipc::data_dir()
        .map(|p| p.join("site-grabber-jobs-v1").join("jobs.json"))
        .ok_or_else(|| io::Error::other("Application data folder unavailable"))
}
fn read(path: &Path) -> io::Result<Disk> {
    if !path.exists() {
        return Ok(empty());
    }
    if std::fs::metadata(path)?.len() > 128 << 20 {
        return Err(io::Error::other("Guarded job store exceeds metadata cap"));
    }
    let disk: Disk = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|_| io::Error::other("Guarded job store is invalid; no jobs were resumed"))?;
    if disk.version != VERSION {
        return Err(io::Error::other("Unsupported guarded job store version"));
    }
    let mut ids = BTreeSet::new();
    for (identity, record) in &disk.records {
        record.guard.validate()?;
        if identity != &record.guard.dispatch_id
            || record.item.id != record.guard.item_id
            || record.item.url != record.guard.url
            || record.item.dest != record.guard.destination
            || !ids.insert(record.item.id)
        {
            return Err(io::Error::other(
                "Guarded job identity or destination mismatch",
            ));
        }
    }
    Ok(disk)
}
fn write(path: &Path, disk: &Disk) -> io::Result<()> {
    write_bounded(path, disk, MAX_METADATA)
}
fn encode_bounded(value: &impl Serialize, maximum: usize) -> io::Result<Vec<u8>> {
    struct Buffer {
        bytes: Vec<u8>,
        maximum: usize,
    }
    impl io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
                return Err(io::Error::other(
                    "Guarded job metadata exceeds its safety cap",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Buffer {
        bytes: Vec::new(),
        maximum,
    };
    serde_json::to_writer(&mut output, value).map_err(io::Error::other)?;
    Ok(output.bytes)
}
fn write_bounded(path: &Path, disk: &Disk, maximum: usize) -> io::Result<()> {
    let bytes = encode_bounded(disk, maximum)?;
    if let Some(parent) = path.parent() {
        sdl_ipc::private_dir(parent)?;
    }
    sdl_ipc::private_write(path, &bytes)
}
pub fn load() -> io::Result<Vec<Item>> {
    load_at(&path()?)
}
/// Includes removed and unacknowledged dispatches, so ordinary jobs never reuse their IDs.
pub fn next_id() -> io::Result<u64> {
    let _lock = LOCK
        .lock()
        .map_err(|_| io::Error::other("Guarded job store unavailable"))?;
    Ok(read(&path()?)?
        .records
        .values()
        .map(|r| r.item.id.saturating_add(1))
        .max()
        .unwrap_or(1))
}

fn load_at(path: &Path) -> io::Result<Vec<Item>> {
    let _lock = LOCK
        .lock()
        .map_err(|_| io::Error::other("Guarded job store unavailable"))?;
    let disk = read(path)?;
    Ok(disk
        .records
        .into_values()
        .filter(|r| !r.removed)
        .map(|mut r| {
            r.item.site = Some(r.guard);
            if r.item.status == Status::Downloading {
                r.item.status = Status::Stopped;
            }
            r.item
        })
        .collect())
}
/// Merge a runtime snapshot on the saver thread; unacknowledged dispatches survive a crash.
pub fn save_items(items: &[Item]) -> io::Result<()> {
    save_items_at(&path()?, items)
}
fn save_items_at(path: &Path, items: &[Item]) -> io::Result<()> {
    let _lock = LOCK
        .lock()
        .map_err(|_| io::Error::other("Guarded job store unavailable"))?;
    let mut disk = read(path)?;
    let present: BTreeSet<_> = items
        .iter()
        .filter_map(|i| i.site.as_ref().map(|s| s.dispatch_id.clone()))
        .collect();
    for (identity, r) in &mut disk.records {
        if r.exposed && !present.contains(identity) {
            r.removed = true;
        }
    }
    for item in items.iter().filter(|i| i.site.is_some()) {
        let guard = item.site.as_ref().unwrap();
        guard.validate()?;
        if item.id != guard.item_id || item.url != guard.url || item.dest != guard.destination {
            return Err(io::Error::other(
                "A guarded job cannot change its URL or reserved destination",
            ));
        }
        let mut saved: Item =
            serde_json::from_slice(&serde_json::to_vec(item).map_err(io::Error::other)?)
                .map_err(io::Error::other)?;
        saved.site = None;
        disk.records.insert(
            guard.dispatch_id.clone(),
            Record {
                guard: guard.clone(),
                item: saved,
                exposed: true,
                removed: false,
            },
        );
    }
    write(path, &disk)
}
pub fn encode_items(items: &[Item]) -> io::Result<Vec<u8>> {
    #[derive(Serialize)]
    struct Runtime<'a> {
        guard: &'a SiteDownloadRequest,
        item: &'a Item,
    }
    encode_bounded(
        &items
            .iter()
            .filter_map(|item| item.site.as_ref().map(|guard| Runtime { guard, item }))
            .collect::<Vec<_>>(),
        MAX_METADATA,
    )
}
pub fn save_encoded(bytes: &[u8]) -> io::Result<()> {
    #[derive(Deserialize)]
    struct Runtime {
        guard: SiteDownloadRequest,
        item: Item,
    }
    let records: Vec<Runtime> = serde_json::from_slice(bytes).map_err(io::Error::other)?;
    let items: Vec<_> = records
        .into_iter()
        .map(|mut r| {
            r.item.site = Some(r.guard);
            r.item
        })
        .collect();
    save_items(&items)
}
pub fn prepare(
    project_id: &ProjectId,
    project: &mut FileProject,
    store: &CatalogStore,
    options: &sdl_engine::Options,
    first_id: u64,
    reserved: BTreeSet<PathBuf>,
) -> io::Result<Vec<u64>> {
    prepare_at(
        &path()?,
        project_id,
        project,
        store,
        options,
        first_id,
        reserved,
    )
}
#[allow(clippy::too_many_arguments)]
fn prepare_at(
    path: &Path,
    project_id: &ProjectId,
    project: &mut FileProject,
    store: &CatalogStore,
    options: &sdl_engine::Options,
    first_id: u64,
    mut reserved: BTreeSet<PathBuf>,
) -> io::Result<Vec<u64>> {
    let _lock = LOCK
        .lock()
        .map_err(|_| io::Error::other("Guarded job store unavailable"))?;
    let mut disk = read(path)?;
    let policy = CapturePolicy::snapshot(project, options)?;
    reserved.extend(disk.records.values().map(|r| r.guard.destination.clone()));
    let mut ids = Vec::new();
    let mut next = first_id;
    let keys: Vec<_> = project
        .candidates
        .iter()
        .filter(|(_, r)| r.checked && project.download_allowed(&r.metadata))
        .map(|(k, _)| k.clone())
        .collect();
    let new_count = keys
        .iter()
        .filter(|key| {
            !disk
                .records
                .contains_key(&SiteDownloadRequest::identity(project_id, key))
        })
        .count();
    let policy_bytes = encode_bounded(&policy, MAX_METADATA)?.len();
    let existing_bytes = std::fs::metadata(path).map_or(0, |m| m.len() as usize);
    if policy_bytes
        .saturating_mul(new_count)
        .saturating_add(existing_bytes)
        > MAX_METADATA
    {
        return Err(io::Error::other(
            "Selected jobs exceed the guarded metadata cap; select fewer files",
        ));
    }
    for key in keys {
        let identity = SiteDownloadRequest::identity(project_id, &key);
        if let Some(existing) = disk.records.get(&identity) {
            let record = project.candidates.get_mut(&key).unwrap();
            record.dispatch_id = Some(identity);
            record.item_id = Some(existing.item.id);
            record.status = TransferStatus::Queued;
            if !existing.removed {
                ids.push(existing.item.id);
            }
            continue;
        }
        let record = &project.candidates[&key];
        if record.dispatch_id.is_some() {
            continue;
        }
        let Some(destination) = sdl_grabber::destinations::reserve(
            &project.config,
            &record.metadata,
            project.layout,
            project.collision,
            &reserved,
        )?
        else {
            continue;
        };
        if disk.records.values().any(|r| r.item.id == next) {
            return Err(io::Error::other(
                "A dispatch ID is already reserved; reload the download list",
            ));
        }
        let guard = SiteDownloadRequest {
            project_id: project_id.clone(),
            candidate_id: key.clone(),
            scan_revision: record.scan_revision,
            url: record.metadata.url.clone(),
            name: record.metadata.name.clone(),
            destination: destination.clone(),
            expected_bytes: record.metadata.size,
            source_page: record.metadata.source_page.clone(),
            policy: policy.clone(),
            dispatch_id: identity.clone(),
            item_id: next,
        };
        guard.validate()?;
        let mut item = Item::new(
            next,
            guard.url.clone(),
            destination.clone(),
            Category::of(&guard.name),
        );
        item.file_name = destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        item.size = guard.expected_bytes;
        item.direct = true;
        disk.records.insert(
            identity.clone(),
            Record {
                guard,
                item,
                exposed: false,
                removed: false,
            },
        );
        reserved.insert(destination);
        ids.push(next);
        next = next
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Download IDs exhausted"))?;
        let record = project.candidates.get_mut(&key).unwrap();
        record.dispatch_id = Some(identity);
        record.item_id = ids.last().copied();
        record.status = TransferStatus::Queued;
    }
    // Persist constraints and reservations before the catalog acknowledges them or UI sees items.
    write(path, &disk)?;
    store.checkpoint(project)?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl_grabber::UrlKey;
    fn setup(root: &Path) -> (ProjectId, FileProject, CatalogStore) {
        let id = ProjectId::parse("files").unwrap();
        let mut p = FileProject::new(sdl_grabber::ProjectConfig::new(
            "Files",
            "https://example.test/series/".parse().unwrap(),
            root.join("output"),
        ));
        for name in ["one.mkv", "two.mp4", "unchecked.mkv"] {
            let url = p.config.seed.join(name).unwrap();
            let key = UrlKey::of(&url);
            p.merge(sdl_grabber::CandidateMetadata {
                url: url.clone(),
                final_url: url,
                name: name.into(),
                mime: Some("video/mp4".into()),
                size: Some(16),
                source_page: p.config.seed.clone(),
            });
            p.candidates.get_mut(&key).unwrap().checked = name != "unchecked.mkv";
        }
        let store = CatalogStore::open(root, &id).unwrap();
        store.checkpoint(&p).unwrap();
        (id, p, store)
    }
    #[test]
    fn site_grabber_checked_jobs_are_durable_and_duplicate_dispatch_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guarded.json");
        let (id, mut p, store) = setup(dir.path());
        let original = store.load().unwrap();
        let first = prepare_at(
            &path,
            &id,
            &mut p,
            &store,
            &Default::default(),
            100,
            Default::default(),
        )
        .unwrap();
        assert_eq!(first.len(), 2);
        // Crash before catalog acknowledgement: replay the old catalog against durable dispatches.
        let mut stale = original;
        let second = prepare_at(
            &path,
            &id,
            &mut stale,
            &store,
            &Default::default(),
            200,
            Default::default(),
        )
        .unwrap();
        assert_eq!(first, second);
        store.delete_metadata().unwrap();
        let items = load_at(&path).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.site.is_some()));
        for item in &items {
            item.site.as_ref().unwrap().validate().unwrap();
        }
        save_items_at(&path, &items).unwrap();
    }
    #[test]
    fn site_grabber_missing_guard_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guarded.json");
        let (id, mut p, store) = setup(dir.path());
        prepare_at(
            &path,
            &id,
            &mut p,
            &store,
            &Default::default(),
            1,
            Default::default(),
        )
        .unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["records"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("guard");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load_at(&path).is_err());
    }

    #[test]
    fn site_grabber_oversized_checkpoint_preserves_durable_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let (id, mut p, store) = setup(dir.path());
        prepare_at(
            &path,
            &id,
            &mut p,
            &store,
            &Default::default(),
            100,
            Default::default(),
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut disk = read(&path).unwrap();
        disk.records.values_mut().next().unwrap().item.description = "x".repeat(4096);
        assert!(write_bounded(&path, &disk, 1024).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(load_at(&path).unwrap().len(), 2);
    }

    #[test]
    fn site_grabber_changed_runtime_url_is_rejected_before_overwriting_the_guard_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let (id, mut p, store) = setup(dir.path());
        prepare_at(
            &path,
            &id,
            &mut p,
            &store,
            &Default::default(),
            100,
            Default::default(),
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut items = load_at(&path).unwrap();
        items[0].url = "https://example.test/series/unselected.mkv"
            .parse()
            .unwrap();
        assert!(save_items_at(&path, &items).is_err());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn site_grabber_dispatch_never_exceeds_the_ids_reserved_by_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let (id, mut p, store) = setup(dir.path());
        prepare_at(
            &path,
            &id,
            &mut p,
            &store,
            &Default::default(),
            100,
            Default::default(),
        )
        .unwrap();
        let items = load_at(&path).unwrap();
        save_items_at(&path, &items).unwrap();
        save_items_at(&path, &[]).unwrap();
        for record in p.candidates.values_mut() {
            record.dispatch_id = None;
            record.item_id = None;
        }
        let other = ProjectId::parse("another-project").unwrap();
        let other_store = CatalogStore::open(dir.path(), &other).unwrap();
        assert!(
            prepare_at(
                &path,
                &other,
                &mut p,
                &other_store,
                &Default::default(),
                100,
                Default::default()
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn site_grabber_local_explore_select_queue_pause_restart_resume_exact_bytes() {
        use sdl_engine::{Destination, Download, Request};
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let bytes: Vec<u8> = (0..131072).map(|n| (n % 251) as u8).collect();
        for name in ["episode.mkv", "episode.mp4", "unchecked.mkv"] {
            std::fs::write(source.join(name), &bytes).unwrap();
        }
        let server = sdl_labserver::LabServer::start(sdl_labserver::Config {
            files: Some(source),
            ..Default::default()
        })
        .await
        .unwrap();
        let id = ProjectId::parse("local-workflow").unwrap();
        let mut project = FileProject::new(sdl_grabber::ProjectConfig::new(
            "Files",
            server.url("/capture/workflow/").parse().unwrap(),
            dir.path().join("Downloads"),
        ));
        project.config.allow_private_seed = true;
        project.config.limits.depth = 1;
        let store = CatalogStore::open(dir.path(), &id).unwrap();
        let transport = sdl_grabber::crawl::CrawlTransport::new(
            Default::default(),
            Arc::new(sdl_engine::crawl_fetch::NetworkAccess::with_private_origin(
                project.config.seed.origin().ascii_serialization(),
            )),
        );
        sdl_grabber::explore::explore_catalog(
            &mut project,
            Some(&store),
            transport,
            Default::default(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(project.candidates.len(), 3);
        assert!(
            !server
                .log()
                .iter()
                .any(|e| e.path.starts_with("/files/") && e.method == "GET" && e.range.is_none())
        );
        for r in project.candidates.values_mut() {
            r.checked = r.metadata.name != "unchecked.mkv";
        }
        let path = dir.path().join("jobs.json");
        let ids = prepare_at(
            &path,
            &id,
            &mut project,
            &store,
            &Default::default(),
            100,
            Default::default(),
        )
        .unwrap();
        assert_eq!(ids.len(), 2);
        let mut jobs = load_at(&path).unwrap();
        let mut first = jobs.remove(0);
        let guard = first.site.as_ref().unwrap();
        let (mut options, runtime) = guard
            .guard(sdl_engine::Options {
                speed_limit: Some(8192),
                save_interval: std::time::Duration::from_millis(30),
                max_connections: 2,
                ..Default::default()
            })
            .unwrap();
        options.capture_guard = Some(runtime);
        let download = Download::start(
            Request {
                url: first.url.clone(),
                headers: Default::default(),
                dest: Destination::File(first.dest.clone()),
            },
            options,
        );
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            while download.snapshot().downloaded < 4096
                || !sdl_engine::state::state_path(&first.dest).exists()
            {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        download.cancel();
        assert!(download.wait().await.is_err());
        first.status = Status::Stopped;
        first.downloaded = 4096;
        jobs.push(first);
        save_items_at(&path, &jobs).unwrap();
        drop(jobs);
        for job in load_at(&path).unwrap() {
            let (mut options, runtime) = job
                .site
                .as_ref()
                .unwrap()
                .guard(Default::default())
                .unwrap();
            options.capture_guard = Some(runtime);
            let state = sdl_engine::state::state_path(&job.dest);
            let download = if state.exists() {
                Download::resume(state, Default::default(), options).unwrap()
            } else {
                Download::start(
                    Request {
                        url: job.url,
                        headers: Default::default(),
                        dest: Destination::File(job.dest.clone()),
                    },
                    options,
                )
            };
            let outcome = tokio::time::timeout(std::time::Duration::from_secs(20), download.wait())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(std::fs::read(&outcome.path).unwrap(), bytes);
            assert!(matches!(
                outcome.path.file_name().unwrap().to_str().unwrap(),
                "episode.mkv" | "episode.mp4"
            ));
        }
        assert!(!dir.path().join("Downloads/unchecked.mkv").exists());
    }

    #[tokio::test]
    #[ignore = "Explicit discovery-only live acceptance; no full episode downloads"]
    async fn site_grabber_live_flash_discovery_with_saved_network_settings() {
        use std::sync::Arc;
        let settings = crate::store::settings_only().unwrap_or_default();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.superpowers/sdd/2026-11-26-idm-site-grabber-parity/live-discovery");
        std::fs::create_dir_all(&root).unwrap();
        let mut project = FileProject::new(sdl_grabber::ProjectConfig::new(
            "Flash discovery",
            "http://203.76.96.50/ftp/TVseries/english/The%20Flash/"
                .parse()
                .unwrap(),
            root.join("no-media-output"),
        ));
        project.config.limits.depth = 1;
        project.config.limits.urls = 64;
        project.config.limits.body_bytes = 2 << 20;
        project.config.limits.parse_bytes = 128 << 10;
        project.config.limits.timeout_secs = 10;
        project.config.limits.attempts = 1;
        project.config.extensions = ["mkv".into(), "mp4".into(), "avi".into()].into();
        let options = sdl_engine::Options {
            proxy: settings.proxy_for(&Default::default()),
            dns: settings.dns(),
            route: sdl_engine::net::route_for(&settings.net_adapter, &crate::netwatch::adapters()),
            ..Default::default()
        };
        let route = match options.proxy {
            sdl_engine::Proxy::None => "Direct",
            sdl_engine::Proxy::System => "System",
            sdl_engine::Proxy::Manual { .. } => "Manual proxy",
            sdl_engine::Proxy::Pac { .. } => "PAC",
        };
        let store =
            CatalogStore::open(&root, &ProjectId::parse("flash-discovery").unwrap()).unwrap();
        let cancel = tokio_util::sync::CancellationToken::new();
        let timeout = cancel.clone();
        let watchdog = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(120)).await;
            timeout.cancel();
        });
        let result = sdl_grabber::explore::explore_catalog(
            &mut project,
            Some(&store),
            sdl_grabber::crawl::CrawlTransport::new(
                options,
                Arc::new(sdl_engine::crawl_fetch::NetworkAccess::public()),
            ),
            cancel,
            None,
        )
        .await;
        watchdog.abort();
        result.unwrap();
        let summary = serde_json::json!({"route":route,"candidates":project.candidates.len(),"known_sizes":project.candidates.values().filter(|r|r.metadata.size.is_some()).count(),"scan_bytes":project.scan_bytes,"status":project.explore_status,"full_episode_downloads":0});
        std::fs::write(
            root.join("summary.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
        let mut report = Vec::new();
        project.write_report(&mut report).unwrap();
        std::fs::write(root.join("candidates.jsonl"), report).unwrap();
        assert!(!project.config.destination.exists());
        assert!(
            !project.candidates.is_empty(),
            "{summary}; reports: {:?}",
            project
                .reports
                .iter()
                .map(|r| &r.reason)
                .collect::<Vec<_>>()
        );
    }
}
