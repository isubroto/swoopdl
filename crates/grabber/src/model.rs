//! Versioned project state. Credentials never belong in this serialized model.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
};
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Limits {
    pub depth: u16,
    pub urls: usize,
    pub body_bytes: u64,
    pub resource_bytes: u64,
    pub parse_bytes: usize,
    pub active: usize,
    pub per_origin: usize,
    pub interval_ms: u64,
    pub attempts: u8,
    pub redirects: u8,
    pub timeout_secs: u64,
    pub robots_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            depth: 3,
            urls: 1000,
            body_bytes: 1 << 30,
            resource_bytes: 256 << 20,
            parse_bytes: 8 << 20,
            active: 4,
            per_origin: 2,
            interval_ms: 1000,
            attempts: 3,
            redirects: 10,
            timeout_secs: 30,
            robots_bytes: 1 << 20,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub seed: Url,
    pub destination: PathBuf,
    pub page_origins: BTreeSet<String>,
    pub resource_origins: BTreeSet<String>,
    pub path_prefix: String,
    pub include_prefixes: Vec<String>,
    pub exclude_prefixes: Vec<String>,
    pub mime_types: BTreeSet<String>,
    pub extensions: BTreeSet<String>,
    /// Explicit intranet consent applies only to the seed origin.
    pub allow_private_seed: bool,
    pub limits: Limits,
}
impl ProjectConfig {
    pub fn new(name: &str, mut seed: Url, destination: PathBuf) -> Self {
        seed.set_fragment(None);
        let origin = seed.origin().ascii_serialization();
        let path_prefix = seed
            .path()
            .rsplit_once('/')
            .map_or("/", |(p, _)| p)
            .to_owned()
            + "/";
        Self {
            name: name.into(),
            seed,
            destination,
            page_origins: BTreeSet::from([origin.clone()]),
            resource_origins: BTreeSet::from([origin]),
            path_prefix,
            include_prefixes: Vec::new(),
            exclude_prefixes: Vec::new(),
            mime_types: BTreeSet::new(),
            extensions: BTreeSet::new(),
            allow_private_seed: false,
            limits: Limits::default(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        crate::normalize_url(&self.seed, self.seed.as_str()).map_err(|e| e.to_string())?;
        let l = &self.limits;
        if self.name.trim().is_empty()
            || self.name.len() > 200
            || self.destination.as_os_str().is_empty()
        {
            return Err("A project needs a name and destination".into());
        }
        if !(1..=100_000).contains(&l.urls)
            || l.body_bytes == 0
            || l.resource_bytes == 0
            || l.parse_bytes == 0
            || l.parse_bytes > 8 << 20
            || l.active == 0
            || l.active > 4
            || l.per_origin == 0
            || l.per_origin > 2
            || l.interval_ms < 1000
            || l.attempts == 0
            || l.attempts > 3
            || l.redirects == 0
            || l.redirects > 10
            || l.timeout_secs == 0
            || l.timeout_secs > 300
            || l.robots_bytes < 512_000
            || l.robots_bytes > 1 << 20
            || l.body_bytes > (1 << 50)
            || l.resource_bytes > (1 << 30)
            || l.interval_ms > 86_400_000
        {
            return Err("Invalid or unsafe crawl limits".into());
        }
        if !self.path_prefix.starts_with('/') || !self.path_prefix.ends_with('/') {
            return Err("The page path scope must be a directory ending in /".into());
        }
        for origin in self.page_origins.iter().chain(&self.resource_origins) {
            let u = Url::parse(origin).map_err(|_| "Invalid origin")?;
            crate::normalize_url(&u, u.as_str()).map_err(|e| e.to_string())?;
            if u.origin().ascii_serialization() != *origin {
                return Err("Origins must contain only scheme, host and optional port".into());
            }
        }
        if self.page_origins.len() > 128
            || self.resource_origins.len() > 128
            || self.include_prefixes.len() + self.exclude_prefixes.len() > 1000
        {
            return Err("Too many origin or filter rules".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectId(String);
impl TryFrom<String> for ProjectId {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl From<ProjectId> for String {
    fn from(value: ProjectId) -> Self {
        value.0
    }
}
impl ProjectId {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err("Invalid project identifier".into());
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UrlKey(pub String);
impl UrlKey {
    pub fn of(url: &Url) -> Self {
        let mut url = url.clone();
        url.set_fragment(None);
        Self(hex::encode(Sha256::digest(url.as_str().as_bytes())))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Page,
    Asset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkRole {
    Navigation,
    File,
    Unresolved,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectMode {
    Files,
    #[default]
    OfflineSite,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateMetadata {
    pub url: Url,
    pub final_url: Url,
    pub name: String,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub source_page: Url,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    Draft,
    Running,
    Pausing,
    Paused,
    Completed,
    Limited,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    Scheme,
    Scope,
    Depth,
    Filter,
    Robots,
    UrlLimit,
    ByteLimit,
    ParseLimit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    pub url: Url,
    pub kind: ResourceKind,
    pub depth: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub url: Url,
    pub final_url: Url,
    pub kind: ResourceKind,
    pub depth: u16,
    pub mime: String,
    pub raw_hash: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub aliases: Vec<Url>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportEntry {
    pub url: Url,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CredentialRef {
    pub identifier: String,
    pub origin: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarSchedule {
    pub hour: u8,
    pub minute: u8,
    pub days: [bool; 7],
    pub once: Option<(i32, u32, u32)>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub version: u32,
    pub config: ProjectConfig,
    pub status: ProjectStatus,
    pub pending: VecDeque<Pending>,
    pub seen: BTreeSet<UrlKey>,
    pub resources: BTreeMap<UrlKey, ResourceRecord>,
    pub reports: Vec<ReportEntry>,
    pub consumed_bytes: u64,
    /// A durable reservation conservatively charged after an interrupted request.
    pub reserved_bytes: u64,
    pub generation: u64,
    pub next_run: Option<u64>,
    pub sync_interval_secs: Option<u64>,
    #[serde(default)]
    pub credential: Option<CredentialRef>,
    #[serde(default)]
    pub schedule: Option<CalendarSchedule>,
    /// Absolute Unix milliseconds; pacing and server retry delays survive restarts.
    #[serde(default)]
    pub next_request_after: Option<u64>,
    /// Latches run limits across pause/restart until an explicit fresh synchronization.
    #[serde(default)]
    pub limit_reached: bool,
}
impl ProjectSnapshot {
    pub fn new(config: ProjectConfig) -> Self {
        let first = Pending {
            url: config.seed.clone(),
            kind: ResourceKind::Page,
            depth: 0,
        };
        Self {
            version: 1,
            config,
            status: ProjectStatus::Draft,
            seen: BTreeSet::from([UrlKey::of(&first.url)]),
            pending: VecDeque::from([first]),
            resources: BTreeMap::new(),
            reports: Vec::new(),
            consumed_bytes: 0,
            reserved_bytes: 0,
            generation: 0,
            next_run: None,
            sync_interval_secs: None,
            credential: None,
            schedule: None,
            next_request_after: None,
            limit_reached: false,
        }
    }
}
