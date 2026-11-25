//! Atomic metadata, immutable raw bodies, and conservative crash recovery.
use crate::{Pending, ProjectId, ProjectSnapshot, ProjectStatus, UrlKey, paths};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
pub type StoreError = io::Error;
const MAX_SNAPSHOT_BYTES: usize = 256 << 20;

/// Read-only preflight for a new project's destination. Existing captures retain
/// their separate ownership check when resuming through `prepare_output`.
pub fn validate_new_destination(root: &Path) -> io::Result<()> {
    let root = paths::checked_path(root, Path::new(""))?;
    if root.exists() && fs::read_dir(&root)?.next().transpose()?.is_some() {
        return Err(io::Error::other(
            "Choose an empty capture destination or a new project subfolder",
        ));
    }
    Ok(())
}

struct BoundedMetadata {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}
impl io::Write for BoundedMetadata {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Project metadata exceeds its safety cap",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod bounded_checkpoint_tests {
    use super::*;
    #[test]
    fn oversized_checkpoint_keeps_the_previous_recoverable_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            ProjectStore::open(dir.path(), &ProjectId::parse("bounded-state").unwrap()).unwrap();
        let mut state = ProjectSnapshot::new(crate::ProjectConfig::new(
            "Before",
            "https://example.test/".parse().unwrap(),
            dir.path().join("output"),
        ));
        store.checkpoint(&state).unwrap();
        let previous = fs::read(store.path()).unwrap();
        state.config.name = "After".into();
        state.config.include_prefixes = vec![format!("https://example.test/{}", "x".repeat(8000))];
        let error = store.checkpoint_bounded(&state, 4096).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
        assert_eq!(fs::read(store.path()).unwrap(), previous);
        assert_eq!(store.load().unwrap().config.name, "Before");
    }
}
#[derive(Clone, Debug)]
pub struct ProjectStore {
    directory: PathBuf,
    id: ProjectId,
}
impl ProjectStore {
    pub fn open(root: &Path, id: &ProjectId) -> Result<Self, StoreError> {
        ProjectId::parse(id.as_str()).map_err(io::Error::other)?;
        let directory = paths::checked_path(root, Path::new(id.as_str()))?;
        fs::create_dir_all(&directory)?;
        Ok(Self {
            directory,
            id: id.clone(),
        })
    }
    pub fn id(&self) -> &ProjectId {
        &self.id
    }
    pub fn path(&self) -> PathBuf {
        self.directory.join("project.json")
    }
    fn backup(&self) -> PathBuf {
        self.directory.join("project.backup.json")
    }
    fn read(&self, path: &Path) -> io::Result<ProjectSnapshot> {
        self.read_bytes(path).map(|(state, _)| state)
    }
    fn read_bytes(&self, path: &Path) -> io::Result<(ProjectSnapshot, Vec<u8>)> {
        paths::checked_path(
            &self.directory,
            path.file_name()
                .ok_or_else(|| io::Error::other("Invalid state path"))?
                .as_ref(),
        )?;
        if fs::metadata(path)?.len() > MAX_SNAPSHOT_BYTES as u64 {
            return Err(io::Error::other("Project metadata exceeds its safety cap"));
        }
        let bytes = fs::read(path)?;
        let state: ProjectSnapshot = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        validate(&state)?;
        Ok((state, bytes))
    }
    pub fn load(&self) -> Result<ProjectSnapshot, StoreError> {
        let mut state = self
            .read(&self.path())
            .or_else(|_| self.read(&self.backup()))?;
        if matches!(
            state.status,
            ProjectStatus::Running | ProjectStatus::Pausing
        ) || state.reserved_bytes > 0
        {
            state.consumed_bytes = state
                .consumed_bytes
                .saturating_add(state.reserved_bytes)
                .min(state.config.limits.body_bytes);
            state.reserved_bytes = 0;
            state.status = ProjectStatus::Paused;
        }
        Ok(state)
    }
    pub fn checkpoint(&self, state: &ProjectSnapshot) -> Result<(), StoreError> {
        self.checkpoint_bounded(state, MAX_SNAPSHOT_BYTES)
    }
    fn checkpoint_bounded(
        &self,
        state: &ProjectSnapshot,
        maximum: usize,
    ) -> Result<(), StoreError> {
        validate(state)?;
        let mut output = BoundedMetadata {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        };
        let result = serde_json::to_writer(&mut output, state);
        if output.exceeded {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "Project metadata exceeds its safety cap",
            ));
        }
        result.map_err(io::Error::other)?;
        paths::checked_path(&self.directory, Path::new("project.json"))?;
        if let Ok((_, previous)) = self.read_bytes(&self.path()) {
            paths::write(&self.directory, Path::new("project.backup.json"), &previous)?;
        }
        paths::write(&self.directory, Path::new("project.json"), &output.bytes)?;
        Ok(())
    }
    pub fn prepare_output(&self, root: &Path) -> io::Result<()> {
        let marker = paths::checked_path(root, Path::new(".swoopdl-project"))?;
        if root.exists() {
            if marker.exists() {
                if fs::read_to_string(&marker)? != self.id.as_str() {
                    return Err(io::Error::other("Destination belongs to another capture"));
                }
            } else {
                validate_new_destination(root)?;
            }
        }
        fs::create_dir_all(root)?;
        paths::write(
            root,
            Path::new(".swoopdl-project"),
            self.id.as_str().as_bytes(),
        )?;
        for name in ["raw", "temporary", "generations"] {
            fs::create_dir_all(paths::checked_path(root, Path::new(name))?)?;
        }
        Ok(())
    }
    pub fn raw_path(&self, root: &Path, hash: &str) -> io::Result<PathBuf> {
        paths::validate_hash(hash)?;
        paths::checked_path(root, &PathBuf::from("raw").join(hash))
    }
    pub fn verified_body(&self, state: &ProjectSnapshot, key: &UrlKey) -> io::Result<Vec<u8>> {
        let record = state
            .resources
            .get(key)
            .ok_or_else(|| io::Error::other("No saved resource"))?;
        let path = self.raw_path(&state.config.destination, &record.raw_hash)?;
        if fs::metadata(&path)?.len() > state.config.limits.resource_bytes {
            return Err(io::Error::other("Saved body exceeds resource cap"));
        }
        let body = fs::read(path)?;
        if hex::encode(Sha256::digest(&body)) != record.raw_hash {
            return Err(io::Error::other("Saved body failed verification"));
        }
        Ok(body)
    }
    pub fn delete_metadata(&self) -> io::Result<()> {
        for name in ["project.json", "project.backup.json"] {
            let p = paths::checked_path(&self.directory, Path::new(name))?;
            if p.exists() {
                fs::remove_file(p)?;
            }
        }
        Ok(())
    }
}
fn validate(state: &ProjectSnapshot) -> io::Result<()> {
    state.config.validate().map_err(io::Error::other)?;
    let limit = state.config.limits.urls;
    if state.version != 1
        || state.seen.len() > limit
        || state.pending.len() > limit
        || state.resources.len() > limit
        || state.reports.len() > limit
        || state.consumed_bytes > state.config.limits.body_bytes
    {
        return Err(io::Error::other("Invalid or unsupported project metadata"));
    }
    for key in state.seen.iter().chain(state.resources.keys()) {
        paths::validate_hash(&key.0)?;
    }
    for record in state.resources.values() {
        paths::validate_hash(&record.raw_hash)?;
    }
    Ok(())
}
pub fn reconcile(
    mut state: ProjectSnapshot,
    output_root: &Path,
) -> Result<ProjectSnapshot, StoreError> {
    if !output_root.is_dir() {
        return Err(io::Error::other(
            "Capture destination is missing. Reconnect it before resuming.",
        ));
    }
    paths::checked_path(output_root, Path::new("raw"))?;
    for record in state.resources.values() {
        let path = paths::checked_path(output_root, &PathBuf::from("raw").join(&record.raw_hash))?;
        if !path.is_file() && !state.pending.iter().any(|p| p.url == record.url) {
            state.pending.push_back(Pending {
                url: record.url.clone(),
                kind: record.kind,
                depth: record.depth,
            });
        }
    }
    Ok(state)
}
