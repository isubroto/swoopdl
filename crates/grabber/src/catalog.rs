//! File projects have their own versioned namespace. Offline snapshot version 1 is unchanged.
use crate::{explore::ExploreEvent, *};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExploreStatus {
    #[default]
    Ready,
    Exploring,
    Stopped,
    Completed,
    Limited,
    Failed,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    #[default]
    Available,
    Queued,
    Downloading,
    Completed,
    Failed,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveLayout {
    #[default]
    Flat,
    OriginalSubfolders,
    Category,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionPolicy {
    #[default]
    Number,
    Skip,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduledAction {
    #[default]
    ExploreOnly,
    ExploreAndDownload,
    DownloadChecked,
    Synchronize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateRecord {
    pub metadata: CandidateMetadata,
    pub checked: bool,
    pub status: TransferStatus,
    pub reason: Option<String>,
    pub item_id: Option<u64>,
    pub dispatch_id: Option<String>,
    pub scan_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanPending {
    pub url: url::Url,
    pub depth: u16,
    pub source_page: url::Url,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileRules {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub minimum_bytes: Option<u64>,
    pub maximum_bytes: Option<u64>,
}
impl FileRules {
    pub fn allows(&self, metadata: &CandidateMetadata) -> bool {
        let url = metadata.final_url.as_str();
        (self.include.is_empty()
            || self
                .include
                .iter()
                .any(|p| wildcard(p, url) || wildcard(p, &metadata.name)))
            && !self
                .exclude
                .iter()
                .any(|p| wildcard(p, url) || wildcard(p, &metadata.name))
            && metadata.size.is_none_or(|n| {
                self.minimum_bytes.is_none_or(|min| n >= min)
                    && self.maximum_bytes.is_none_or(|max| n <= max)
            })
    }
    pub fn validate(&self) -> io::Result<()> {
        if self
            .minimum_bytes
            .zip(self.maximum_bytes)
            .is_some_and(|(min, max)| min > max)
            || self.include.len() + self.exclude.len() > 1000
            || self
                .include
                .iter()
                .chain(&self.exclude)
                .any(|p| p.len() > 8192)
        {
            return Err(io::Error::other("Invalid file patterns or size range"));
        }
        Ok(())
    }
}
pub fn wildcard(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut i, mut j, mut star, mut retry) = (0, 0, None, 0);
    while j < t.len() {
        if i < p.len() && (p[i] == b'?' || p[i].eq_ignore_ascii_case(&t[j])) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == b'*' {
            star = Some(i);
            i += 1;
            retry = j;
        } else if let Some(s) = star {
            retry += 1;
            j = retry;
            i = s + 1;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == b'*' {
        i += 1;
    }
    i == p.len()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileProject {
    pub version: u32,
    pub mode: ProjectMode,
    pub config: ProjectConfig,
    pub scan_revision: u64,
    pub explore_status: ExploreStatus,
    pub candidates: BTreeMap<UrlKey, CandidateRecord>,
    pub frontier: VecDeque<ScanPending>,
    pub seen: BTreeSet<UrlKey>,
    pub scan_bytes: u64,
    pub reserved_bytes: u64,
    pub auto_download: bool,
    pub rules: FileRules,
    pub layout: SaveLayout,
    pub collision: CollisionPolicy,
    pub credential: Option<CredentialRef>,
    pub schedule: Option<CalendarSchedule>,
    pub scheduled_action: ScheduledAction,
    pub next_run: Option<u64>,
    pub reports: Vec<ReportEntry>,
}
impl FileProject {
    pub fn new(mut config: ProjectConfig) -> Self {
        if config.limits.body_bytes == Limits::default().body_bytes {
            config.limits.body_bytes = 64 << 20;
        }
        let first = ScanPending {
            url: config.seed.clone(),
            depth: 0,
            source_page: config.seed.clone(),
        };
        Self {
            version: 1,
            mode: ProjectMode::Files,
            config,
            scan_revision: 1,
            explore_status: ExploreStatus::Ready,
            candidates: BTreeMap::new(),
            frontier: [first].into(),
            seen: BTreeSet::new(),
            scan_bytes: 0,
            reserved_bytes: 0,
            auto_download: false,
            rules: Default::default(),
            layout: Default::default(),
            collision: Default::default(),
            credential: None,
            schedule: None,
            scheduled_action: Default::default(),
            next_run: None,
            reports: Vec::new(),
        }
    }
    pub fn merge(&mut self, metadata: CandidateMetadata) {
        if !self.rules.allows(&metadata) {
            return;
        }
        let key = UrlKey::of(&metadata.url);
        if let Some(record) = self.candidates.get_mut(&key) {
            // Already dispatched jobs keep their original immutable metadata and destination.
            if record.dispatch_id.is_none() {
                record.metadata = metadata;
                record.scan_revision = self.scan_revision;
            }
        } else if self.candidates.len() < self.config.limits.urls {
            let checked = self.auto_download && !crate::classify::is_manifest(&metadata);
            self.candidates.insert(
                key,
                CandidateRecord {
                    metadata,
                    checked,
                    status: TransferStatus::Available,
                    reason: None,
                    item_id: None,
                    dispatch_id: None,
                    scan_revision: self.scan_revision,
                },
            );
        }
    }
    pub fn selected_totals(&self) -> (usize, u64, usize) {
        self.candidates.values().filter(|r| r.checked).fold(
            (0, 0u64, 0),
            |(count, bytes, unknown), r| {
                (
                    count + 1,
                    bytes.saturating_add(r.metadata.size.unwrap_or(0)),
                    unknown + usize::from(r.metadata.size.is_none()),
                )
            },
        )
    }
    pub fn download_allowed(&self, metadata: &CandidateMetadata) -> bool {
        !crate::classify::is_manifest(metadata)
            && self.rules.allows(metadata)
            && evaluate(&self.config, &metadata.final_url, ResourceKind::Asset, 0)
                == Decision::Allow
            && crate::policy::save_allowed(
                &self.config,
                &metadata.final_url,
                ResourceKind::Asset,
                metadata.mime.as_deref().unwrap_or(""),
            )
    }
    pub fn check_all(&mut self, checked: bool) {
        let allowed: BTreeSet<_> = self
            .candidates
            .iter()
            .filter(|(_, r)| r.dispatch_id.is_some() || self.download_allowed(&r.metadata))
            .map(|(key, _)| key.clone())
            .collect();
        for (key, r) in &mut self.candidates {
            r.checked = checked && allowed.contains(key);
        }
    }
    pub fn rescan(&mut self) {
        let excluded: Vec<_> = self
            .candidates
            .iter()
            .filter(|(_, r)| r.dispatch_id.is_none() && !self.download_allowed(&r.metadata))
            .map(|(key, _)| key.clone())
            .collect();
        for key in excluded {
            let r = self.candidates.get_mut(&key).unwrap();
            r.checked = false;
            r.reason = Some("Does not match current project filters".into());
        }
        self.scan_revision = self.scan_revision.saturating_add(1);
        self.frontier = [ScanPending {
            url: self.config.seed.clone(),
            depth: 0,
            source_page: self.config.seed.clone(),
        }]
        .into();
        self.seen.clear();
        self.scan_bytes = 0;
        self.reserved_bytes = 0;
        self.reports.clear();
        self.explore_status = ExploreStatus::Ready;
    }
    pub fn apply(&mut self, event: &ExploreEvent) {
        match event {
            ExploreEvent::CandidateFound(metadata) => self.merge(metadata.as_ref().clone()),
            ExploreEvent::Skipped(url, reason) if self.reports.len() < self.config.limits.urls => {
                self.reports.push(ReportEntry {
                    url: url.clone(),
                    reason: reason.clone(),
                })
            }
            ExploreEvent::Stopped { bytes, limited } => {
                self.scan_bytes = *bytes;
                self.explore_status = if *limited {
                    ExploreStatus::Limited
                } else {
                    ExploreStatus::Stopped
                };
            }
            _ => {}
        }
    }
    pub fn write_report(&self, out: &mut dyn Write) -> io::Result<()> {
        for r in self.candidates.values() {
            serde_json::to_writer(&mut *out, &serde_json::json!({ "url": crate::report::sanitized_url(&r.metadata.url), "source": crate::report::sanitized_url(&r.metadata.source_page), "size": r.metadata.size, "checked":r.checked, "status":r.status })).map_err(io::Error::other)?;
            writeln!(out)?;
        }
        Ok(())
    }
    pub fn validate(&self) -> io::Result<()> {
        if self.version != 1 {
            return Err(io::Error::other("Unsupported newer file-project catalog"));
        }
        if self.mode != ProjectMode::Files {
            return Err(io::Error::other("Invalid file-project mode"));
        }
        if self
            .credential
            .as_ref()
            .is_some_and(|r| r.origin != self.config.seed.origin().ascii_serialization())
        {
            return Err(io::Error::other(
                "Session origin does not match the project",
            ));
        }
        self.config.validate().map_err(io::Error::other)?;
        self.rules.validate()?;
        if self.candidates.len() > self.config.limits.urls
            || self.frontier.len() > self.config.limits.urls
            || self.seen.len() > self.config.limits.urls
            || self.reports.len() > self.config.limits.urls
        {
            return Err(io::Error::other("Catalog exceeds URL limit"));
        }
        for (key, r) in &self.candidates {
            if *key != UrlKey::of(&r.metadata.url) {
                return Err(io::Error::other("Invalid candidate identity"));
            }
            for u in [
                &r.metadata.url,
                &r.metadata.final_url,
                &r.metadata.source_page,
            ] {
                normalize_url(u, u.as_str()).map_err(io::Error::other)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CatalogStore {
    directory: PathBuf,
}
impl CatalogStore {
    pub fn open(root: &Path, id: &ProjectId) -> io::Result<Self> {
        let namespace = paths::checked_path(root, Path::new("file-projects-v1"))?;
        let directory = paths::checked_path(&namespace, Path::new(id.as_str()))?;
        std::fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }
    pub fn path(&self) -> PathBuf {
        self.directory.join("catalog.json")
    }
    fn read(&self, name: &str) -> io::Result<FileProject> {
        let path = paths::checked_path(&self.directory, Path::new(name))?;
        if std::fs::metadata(&path)?.len() > 64 << 20 {
            return Err(io::Error::other("Catalog exceeds metadata cap"));
        }
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path)?).map_err(io::Error::other)?;
        if value
            .get("version")
            .and_then(|v| v.as_u64())
            .is_some_and(|v| v != 1)
        {
            return Err(io::Error::other("Unsupported newer file-project catalog"));
        }
        let state: FileProject = serde_json::from_value(value).map_err(io::Error::other)?;
        state.validate()?;
        Ok(state)
    }
    pub fn load(&self) -> io::Result<FileProject> {
        let mut state = match self.read("catalog.json") {
            Ok(state) => state,
            Err(e)
                if e.to_string().contains("newer")
                    || e.kind() == io::ErrorKind::PermissionDenied =>
            {
                return Err(e);
            }
            Err(_) => self.read("catalog.backup.json")?,
        };
        if state.explore_status == ExploreStatus::Exploring {
            state.explore_status = ExploreStatus::Stopped;
            state.scan_bytes = state
                .scan_bytes
                .saturating_add(state.reserved_bytes)
                .min(state.config.limits.body_bytes);
            state.reserved_bytes = 0;
        }
        Ok(state)
    }
    pub fn checkpoint(&self, state: &FileProject) -> io::Result<()> {
        state.validate()?;
        let bytes = serde_json::to_vec(state).map_err(io::Error::other)?;
        if bytes.len() > 64 << 20 {
            return Err(io::Error::other("Catalog exceeds metadata cap"));
        }
        match self.read("catalog.json") {
            Ok(previous) => {
                paths::write(
                    &self.directory,
                    Path::new("catalog.backup.json"),
                    &serde_json::to_vec(&previous).map_err(io::Error::other)?,
                )?;
            }
            Err(error) if error.to_string().contains("newer") => return Err(error),
            Err(_) => {}
        }
        paths::write(&self.directory, Path::new("catalog.json"), &bytes)?;
        Ok(())
    }
    pub fn delete_metadata(&self) -> io::Result<()> {
        for name in ["catalog.json", "catalog.backup.json"] {
            let p = paths::checked_path(&self.directory, Path::new(name))?;
            if p.exists() {
                std::fs::remove_file(p)?;
            }
        }
        Ok(())
    }
}
