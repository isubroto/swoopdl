//! Saved website projects. Disk, vault and crawler work always runs off the UI thread.
use crate::{dialogs::Outcome, i18n::t};
use chrono::{Datelike, TimeZone};
use sdl_grabber::{
    CalendarSchedule, CredentialRef, ProjectConfig, ProjectId, ProjectSnapshot, ProjectStatus,
    crawl::{CrawlCommand, CrawlEvent, CrawlTransport},
    store::ProjectStore,
};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;
mod view;
mod wizard;
use wizard::{ProjectTemplate, WizardStep};

pub enum SiteAction {
    BrowseDestination,
    ValidateDestination(PathBuf),
    ChangeDestination(ProjectId),
    Create(Box<ProjectConfig>),
    Start(ProjectId),
    Pause(ProjectId),
    Synchronize(ProjectId),
    OpenOutput(ProjectId),
    ExportReport(ProjectId),
    DeleteMetadata(ProjectId),
}
struct Wizard {
    step: WizardStep,
    template: ProjectTemplate,
    name: String,
    seed: String,
    destination: String,
    project_subfolder: bool,
    project_folder: String,
    path_prefix: String,
    page_origins: String,
    asset_origins: String,
    includes: String,
    excludes: String,
    extensions: String,
    mimes: String,
    depth: u16,
    urls: usize,
    budget_mib: u64,
    resource_mib: u64,
    restrict_directory: bool,
    private_seed: bool,
    authenticated: bool,
    authorization: String,
    protect: bool,
    scheduled: bool,
    hour: u8,
    minute: u8,
    days: [bool; 7],
    once: String,
}
impl Default for Wizard {
    fn default() -> Self {
        Self {
            step: WizardStep::Project,
            template: ProjectTemplate::OfflineSite,
            name: String::new(),
            seed: String::new(),
            destination: String::new(),
            project_subfolder: true,
            project_folder: String::new(),
            path_prefix: String::new(),
            page_origins: String::new(),
            asset_origins: String::new(),
            includes: String::new(),
            excludes: String::new(),
            extensions: String::new(),
            mimes: String::new(),
            depth: 3,
            urls: 1000,
            budget_mib: 1024,
            resource_mib: 256,
            restrict_directory: true,
            private_seed: false,
            authenticated: false,
            authorization: String::new(),
            protect: false,
            scheduled: false,
            hour: 23,
            minute: 0,
            days: [true; 7],
            once: String::new(),
        }
    }
}
impl Wizard {
    fn config(&self) -> Result<ProjectConfig, String> {
        let seed = Url::parse(self.seed.trim())
            .map_err(|_| t("Enter a valid HTTP(S) start URL").to_owned())?;
        let mut config = ProjectConfig::new(self.name.trim(), seed, self.destination_path()?);
        config.limits.depth = self.depth;
        config.limits.urls = self.urls;
        config.limits.body_bytes = self
            .budget_mib
            .checked_mul(1 << 20)
            .ok_or_else(|| t("Invalid body budget").to_owned())?;
        config.limits.resource_bytes = self
            .resource_mib
            .checked_mul(1 << 20)
            .ok_or_else(|| t("Invalid maximum file size").to_owned())?;
        config.allow_private_seed = self.private_seed;
        if !self.restrict_directory {
            config.path_prefix = "/".into();
        }
        if !self.path_prefix.trim().is_empty() {
            config.path_prefix = self.path_prefix.trim().into();
        }
        config
            .resource_origins
            .extend(self.asset_origins.split_whitespace().map(str::to_owned));
        config
            .page_origins
            .extend(self.page_origins.split_whitespace().map(str::to_owned));
        config.include_prefixes = self
            .includes
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        config.exclude_prefixes = self
            .excludes
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        config.extensions = self
            .extensions
            .split_whitespace()
            .map(|s| s.trim_start_matches('.').to_ascii_lowercase())
            .collect();
        config.mime_types = self.mimes.split_whitespace().map(str::to_owned).collect();
        config.validate()?;
        if self.authenticated {
            self.authorization
                .parse::<reqwest::header::HeaderValue>()
                .map_err(|_| t("Invalid authorization header").to_owned())?;
            if self.authorization.is_empty() {
                return Err(t("Enter session credentials").into());
            }
        }
        self.schedule()?;
        Ok(config)
    }
    fn schedule(&self) -> Result<Option<CalendarSchedule>, String> {
        if !self.scheduled {
            return Ok(None);
        }
        if self.hour > 23 || self.minute > 59 || !self.days.iter().any(|d| *d) {
            return Err(t("Choose a valid schedule time and day").into());
        }
        let once = if self.once.trim().is_empty() {
            None
        } else {
            let date = chrono::NaiveDate::parse_from_str(self.once.trim(), "%Y-%m-%d")
                .map_err(|_| t("Use YYYY-MM-DD for a one-time schedule").to_owned())?;
            Some((date.year(), date.month(), date.day()))
        };
        Ok(Some(CalendarSchedule {
            hour: self.hour,
            minute: self.minute,
            days: self.days,
            once,
        }))
    }
}
struct ProjectView {
    id: ProjectId,
    state: ProjectSnapshot,
    pause_pending: bool,
    error: Option<String>,
}
impl ProjectView {
    fn apply_result(&mut self, result: Result<ProjectSnapshot, String>) {
        self.pause_pending = false;
        match result {
            Ok(state) => {
                self.state = state;
                self.error = None;
            }
            Err(error) => {
                self.state.status = ProjectStatus::Failed;
                self.state.next_run = None;
                self.error = Some(error);
            }
        }
    }
    #[cfg(test)]
    fn fixture() -> Self {
        Self {
            id: ProjectId::parse("docs").unwrap(),
            state: ProjectSnapshot::new(ProjectConfig::new(
                "Docs",
                Url::parse("https://example.test/").unwrap(),
                "output".into(),
            )),
            pause_pending: false,
            error: None,
        }
    }
}
enum Message {
    FolderPicked(Option<PathBuf>),
    DestinationChecked(PathBuf, Result<(), String>),
    Relocated(ProjectId, Result<Option<ProjectSnapshot>, String>),
    Loaded(Vec<ProjectView>, Vec<String>),
    Created(ProjectView),
    Finished(ProjectId, Result<ProjectSnapshot, String>),
    Removed(ProjectId),
    Error(String),
}
struct Active {
    id: ProjectId,
    commands: tokio::sync::mpsc::Sender<CrawlCommand>,
    events: tokio::sync::mpsc::Receiver<CrawlEvent>,
    saved: usize,
    pending: usize,
}
pub struct SiteGrabber {
    pub open: bool,
    projects: Vec<ProjectView>,
    wizard: Wizard,
    creating: bool,
    folder_pending: bool,
    validating_destination: bool,
    saving: bool,
    loaded: bool,
    selected: Option<ProjectId>,
    error: Option<String>,
    active: Option<Active>,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    session: String,
    memory: BTreeMap<ProjectId, String>,
}
impl Drop for SiteGrabber {
    fn drop(&mut self) {
        if let Some(active) = &self.active {
            let _ = active.commands.try_send(CrawlCommand::Pause);
        }
    }
}
const CREDENTIAL_SERVICE: &str = "Swoopdl Site Grabber";
fn root() -> Result<PathBuf, String> {
    sdl_ipc::data_dir()
        .map(|p| p.join("site-grabber"))
        .ok_or_else(|| t("The application data folder is unavailable").into())
}
fn store(id: &ProjectId) -> Result<ProjectStore, String> {
    ProjectStore::open(&root()?, id).map_err(|e| e.to_string())
}
fn can_change_destination(state: &ProjectSnapshot) -> bool {
    matches!(
        state.status,
        ProjectStatus::Draft | ProjectStatus::Paused | ProjectStatus::Failed
    ) && state.resources.is_empty()
        && state.generation == 0
        && state.consumed_bytes == 0
        && state.reserved_bytes == 0
}
fn relocate_uncaptured(
    store: &ProjectStore,
    base: &std::path::Path,
) -> Result<ProjectSnapshot, String> {
    let mut state = store.load().map_err(|e| e.to_string())?;
    if !can_change_destination(&state) {
        return Err(t("Only projects with no captured data can change destination.").into());
    }
    let destination = base.join(wizard::project_folder_name(&state.config.name));
    sdl_grabber::store::validate_new_destination(&destination).map_err(|e| e.to_string())?;
    state.config.destination = destination;
    state.status = ProjectStatus::Draft;
    state.next_run = None;
    store.checkpoint(&state).map_err(|e| e.to_string())?;
    Ok(state)
}
fn save_credential(vault: &dyn crate::secrets::Store, id: &str, value: &str) -> Result<(), String> {
    vault.probe(CREDENTIAL_SERVICE).map_err(|_| {
        t("Unlock the system credential vault or choose memory-only credentials").to_owned()
    })?;
    vault
        .write(CREDENTIAL_SERVICE, id, "Swoopdl site capture", value)
        .map_err(|_| t("The system credential vault refused to save the session").to_owned())?;
    if vault.read(CREDENTIAL_SERVICE, id).ok().flatten().as_deref() != Some(value) {
        let _ = vault.delete(CREDENTIAL_SERVICE, id);
        return Err(t("The system credential vault could not verify the session").into());
    }
    Ok(())
}
/// Reuse the scheduler's weekday/date selection, resolving each date in local time.
fn next_schedule(schedule: &CalendarSchedule, now: chrono::DateTime<chrono::Local>) -> Option<u64> {
    let calendar = crate::queue::Schedule {
        start_at: Some((schedule.hour, schedule.minute, 0)),
        days: schedule.days,
        once: schedule.once,
        ..Default::default()
    };
    let first = if let Some((y, m, d)) = schedule.once {
        let date = chrono::NaiveDate::from_ymd_opt(y, m, d)?;
        let days = date.signed_duration_since(now.date_naive()).num_days();
        if days < 0 {
            return None;
        }
        days
    } else {
        0
    };
    let last = if schedule.once.is_some() { first } else { 7 };
    for offset in first..=last {
        let date = now
            .date_naive()
            .checked_add_signed(chrono::Duration::days(offset))?;
        if !calendar.starts_on(
            (date.year(), date.month(), date.day()),
            date.weekday().num_days_from_sunday() as usize,
        ) {
            continue;
        }
        let at = date.and_hms_opt(schedule.hour.into(), schedule.minute.into(), 0)?;
        let candidates = match chrono::Local.from_local_datetime(&at) {
            chrono::LocalResult::Single(t) => vec![t],
            chrono::LocalResult::Ambiguous(a, _) => vec![a],
            chrono::LocalResult::None => Vec::new(),
        };
        if let Some(at) = candidates.into_iter().filter(|at| *at > now).min() {
            return u64::try_from(at.timestamp()).ok();
        }
    }
    None
}
fn schedule_next(state: &mut ProjectSnapshot) {
    if let Some(schedule) = &state.schedule {
        state.next_run = next_schedule(schedule, chrono::Local::now());
    }
}
fn should_synchronize(state: &ProjectSnapshot, explicit: bool) -> bool {
    explicit
        || (state.pending.is_empty()
            && matches!(
                state.status,
                ProjectStatus::Completed | ProjectStatus::Limited | ProjectStatus::Failed
            ))
}
impl SiteGrabber {
    pub fn new(rt: &tokio::runtime::Handle) -> Self {
        let (tx, rx) = mpsc::channel();
        let out = tx.clone();
        rt.spawn_blocking(move || {
            let mut projects = Vec::new();
            let mut errors = Vec::new();
            match root() {
                Ok(root) => match std::fs::read_dir(&root) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            let Ok(id) = ProjectId::parse(&entry.file_name().to_string_lossy())
                            else {
                                continue;
                            };
                            match ProjectStore::open(&root, &id).and_then(|s| s.load()) {
                                Ok(state) => projects.push(ProjectView {
                                    id,
                                    state,
                                    pause_pending: false,
                                    error: None,
                                }),
                                Err(e) => errors.push(e.to_string()),
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => errors.push(e.to_string()),
                },
                Err(e) => errors.push(e),
            }
            let _ = out.send(Message::Loaded(projects, errors));
        });
        Self {
            open: false,
            projects: Vec::new(),
            wizard: Wizard::default(),
            creating: false,
            folder_pending: false,
            validating_destination: false,
            saving: false,
            loaded: false,
            selected: None,
            error: None,
            active: None,
            tx,
            rx,
            session: String::new(),
            memory: BTreeMap::new(),
        }
    }
    pub fn seed(&self, action: &SiteAction) -> Option<Url> {
        let id = match action {
            SiteAction::Start(id) | SiteAction::Synchronize(id) => id,
            _ => return None,
        };
        self.projects
            .iter()
            .find(|p| p.id == *id)
            .map(|p| p.state.config.seed.clone())
    }
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<SiteAction> {
        for _ in 0..64 {
            let Ok(message) = self.rx.try_recv() else {
                break;
            };
            match message {
                Message::DestinationChecked(path, result) => {
                    self.validating_destination = false;
                    if self.creating && self.open {
                        self.error = self.wizard.accept_destination_check(&path, result).err();
                    }
                }
                Message::Relocated(id, result) => {
                    self.folder_pending = false;
                    if let Some(project) = self.projects.iter_mut().find(|p| p.id == id) {
                        match result {
                            Ok(Some(state)) => project.apply_result(Ok(state)),
                            Ok(None) => {}
                            Err(error) => project.error = Some(error),
                        }
                    }
                }
                Message::FolderPicked(path) => {
                    self.folder_pending = false;
                    if let Some(path) = path {
                        self.wizard.destination = path.to_string_lossy().into_owned();
                    }
                }
                Message::Loaded(projects, errors) => {
                    self.projects = projects;
                    self.loaded = true;
                    if !errors.is_empty() {
                        self.error = Some(format!(
                            "{}: {}",
                            t("Some projects could not be loaded"),
                            errors.join("; ")
                        ));
                    }
                }
                Message::Created(project) => {
                    self.saving = false;
                    self.selected = Some(project.id.clone());
                    self.projects.push(project);
                    self.creating = false;
                }
                Message::Finished(id, result) => {
                    if let Some(p) = self.projects.iter_mut().find(|p| p.id == id) {
                        p.apply_result(result);
                    }
                    self.active = None;
                }
                Message::Removed(id) => {
                    self.projects.retain(|p| p.id != id);
                    self.memory.remove(&id);
                    self.selected = None;
                }
                Message::Error(error) => {
                    self.saving = false;
                    self.error = Some(error);
                }
            }
            ctx.request_repaint();
        }
        if let Some(active) = &mut self.active {
            for _ in 0..64 {
                let Ok(event) = active.events.try_recv() else {
                    break;
                };
                active.saved = event.saved;
                active.pending = event.pending;
                if let Some(p) = self.projects.iter_mut().find(|p| p.id == active.id) {
                    p.state.consumed_bytes = event.bytes;
                    // A Paused event follows a checkpoint, but the full result also restores the frontier.
                    if event.status != ProjectStatus::Paused {
                        p.state.status = event.status;
                    }
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        } else if self.loaded
            && let Some(p) = self
                .projects
                .iter()
                .find(|p| sdl_grabber::sync::due_run(&p.state, SystemTime::now()))
        {
            return Some(SiteAction::Synchronize(p.id.clone()));
        }
        None
    }
    pub fn dispatch(
        &mut self,
        action: SiteAction,
        rt: &tokio::runtime::Handle,
        options: sdl_engine::Options,
        ctx: &egui::Context,
    ) {
        let out = self.tx.clone();
        let repaint = ctx.clone();
        let operation = match &action {
            SiteAction::OpenOutput(_) => 0,
            SiteAction::ExportReport(_) => 1,
            _ => 2,
        };
        let explicit_sync = matches!(&action, SiteAction::Synchronize(_));
        match action {
            SiteAction::ValidateDestination(path) => {
                if self.validating_destination || self.folder_pending || self.saving {
                    return;
                }
                self.validating_destination = true;
                rt.spawn_blocking(move || {
                    let result = sdl_grabber::store::validate_new_destination(&path)
                        .map_err(|e| e.to_string());
                    let _ = out.send(Message::DestinationChecked(path, result));
                    repaint.request_repaint();
                });
            }
            SiteAction::ChangeDestination(id) => {
                if self.active.is_some() || self.folder_pending || self.validating_destination {
                    return;
                }
                let Some(project) = self
                    .projects
                    .iter()
                    .find(|p| p.id == id && can_change_destination(&p.state))
                else {
                    return;
                };
                let directory = project
                    .state
                    .config
                    .destination
                    .parent()
                    .unwrap_or(&project.state.config.destination)
                    .to_path_buf();
                self.folder_pending = true;
                let title = t("Choose a base folder for this project").to_owned();
                rt.spawn_blocking(move || {
                    let result = (|| {
                        let Some(base) = rfd::FileDialog::new()
                            .set_title(title)
                            .set_directory(directory)
                            .pick_folder()
                        else {
                            return Ok(None);
                        };
                        relocate_uncaptured(&store(&id)?, &base).map(Some)
                    })();
                    let _ = out.send(Message::Relocated(id, result));
                    repaint.request_repaint();
                });
            }
            SiteAction::BrowseDestination => {
                if self.folder_pending || self.validating_destination || self.saving {
                    return;
                }
                self.folder_pending = true;
                let title = t("Choose capture folder").to_owned();
                let directory = self.wizard.destination.clone();
                rt.spawn_blocking(move || {
                    let mut dialog = rfd::FileDialog::new().set_title(title);
                    if !directory.is_empty() {
                        dialog = dialog.set_directory(directory);
                    }
                    let _ = out.send(Message::FolderPicked(dialog.pick_folder()));
                    repaint.request_repaint();
                });
            }
            SiteAction::Create(config) => {
                if self.saving {
                    return;
                }
                self.saving = true;
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let id = ProjectId::parse(&format!("site-{stamp}")).unwrap();
                let auth = self
                    .wizard
                    .authenticated
                    .then(|| std::mem::take(&mut self.wizard.authorization));
                let protected = self.wizard.protect;
                let schedule = self.wizard.schedule().ok().flatten();
                if let Some(value) = &auth
                    && !protected
                {
                    self.memory.insert(id.clone(), value.clone());
                }
                rt.spawn_blocking(move || {
                    let result = (|| {
                        config.validate()?;
                        sdl_grabber::store::validate_new_destination(&config.destination)
                            .map_err(|e| e.to_string())?;
                        let store = store(&id)?;
                        let mut state = ProjectSnapshot::new(*config);
                        state.schedule = schedule;
                        schedule_next(&mut state);
                        if let Some(value) = auth {
                            if protected {
                                save_credential(&crate::secrets::System, id.as_str(), &value)?;
                            }
                            state.credential = Some(CredentialRef {
                                identifier: if protected {
                                    id.as_str().into()
                                } else {
                                    "memory-only".into()
                                },
                                origin: state.config.seed.origin().ascii_serialization(),
                            });
                        }
                        store.checkpoint(&state).map_err(|e| e.to_string())?;
                        Ok::<_, String>(ProjectView {
                            id,
                            state,
                            pause_pending: false,
                            error: None,
                        })
                    })();
                    let _ = out.send(match result {
                        Ok(p) => Message::Created(p),
                        Err(e) => Message::Error(e),
                    });
                    repaint.request_repaint();
                });
            }
            SiteAction::Start(id) | SiteAction::Synchronize(id) => {
                if self.folder_pending || self.validating_destination {
                    return;
                }
                if self.active.is_some() {
                    self.error = Some(t("Pause the active project before starting another").into());
                    return;
                }
                let Some(project) = self.projects.iter_mut().find(|p| p.id == id) else {
                    return;
                };
                let synchronize = should_synchronize(&project.state, explicit_sync);
                let saved = project.state.resources.len();
                let pending = project.state.pending.len();
                if !self.session.is_empty() {
                    self.memory
                        .insert(id.clone(), std::mem::take(&mut self.session));
                }
                let memory = self.memory.get(&id).cloned();
                let (commands, rx) = tokio::sync::mpsc::channel(2);
                let (events, event_rx) = tokio::sync::mpsc::channel(32);
                project.state.status = ProjectStatus::Running;
                project.error = None;
                self.active = Some(Active {
                    id: id.clone(),
                    commands,
                    events: event_rx,
                    saved,
                    pending,
                });
                let handle = rt.clone();
                rt.spawn_blocking(move || {
                    let result = (|| {
                        let store = store(&id)?;
                        let state = store.load().map_err(|e| e.to_string())?;
                        let network = if state.config.allow_private_seed { sdl_engine::crawl_fetch::NetworkAccess::with_private_origin(state.config.seed.origin().ascii_serialization()) } else { sdl_engine::crawl_fetch::NetworkAccess::public() };
                        let mut transport = CrawlTransport::new(options, Arc::new(network));
                        if let Some(reference) = &state.credential {
                            if reference.origin != state.config.seed.origin().ascii_serialization() { return Err(t("Session origin does not match the project").into()); }
                            let value = match memory {
                                Some(value) => value,
                                None if reference.identifier != "memory-only" => crate::secrets::Store::read(&crate::secrets::System, CREDENTIAL_SERVICE, &reference.identifier).ok().flatten().ok_or_else(|| t("Unlock the credential vault or enter a replacement session").to_owned())?,
                                None => return Err(t("Enter a replacement memory-only session before resuming").into()),
                            };
                            let mut header = value.parse::<reqwest::header::HeaderValue>().map_err(|_| t("Invalid authorization header").to_owned())?;
                            header.set_sensitive(true);
                            transport.headers.insert(reqwest::header::AUTHORIZATION, header);
                            transport.credential_origin = Some(reference.origin.clone());
                        }
                        let mut final_state = handle.block_on(async {
                            if synchronize { sdl_grabber::sync::synchronize(store.clone(), transport, rx, events).await }
                            else { sdl_grabber::crawl::run(state.config.clone(), state, store.clone(), transport, rx, events).await }
                        }).map_err(|e| e.to_string())?;
                        if final_state.status != ProjectStatus::Paused { schedule_next(&mut final_state); store.checkpoint(&final_state).map_err(|e| e.to_string())?; }
                        Ok(final_state)
                    })();
                    if result.is_err() && let Ok(store) = store(&id) && let Ok(mut state) = store.load() { state.status = ProjectStatus::Failed; state.next_run = None; let _ = store.checkpoint(&state); }
                    let _ = out.send(Message::Finished(id, result)); repaint.request_repaint();
                });
            }
            SiteAction::Pause(id) => {
                if let Some(active) = &self.active
                    && active.id == id
                    && active.commands.try_send(CrawlCommand::Pause).is_ok()
                    && let Some(p) = self.projects.iter_mut().find(|p| p.id == id)
                {
                    p.pause_pending = true;
                }
            }
            SiteAction::OpenOutput(id)
            | SiteAction::ExportReport(id)
            | SiteAction::DeleteMetadata(id) => {
                if self.active.as_ref().is_some_and(|p| p.id == id) {
                    return;
                }
                let Some(project) = self.projects.iter().find(|p| p.id == id) else {
                    return;
                };
                let destination = project.state.config.destination.clone();
                // The requested operation is carried separately because this match shares path lookup.
                rt.spawn_blocking(move || {
                    let result = (|| {
                        let store = store(&id)?;
                        match operation {
                            0 => {
                                open::that(destination.join("index.html"))
                                    .map_err(|e| e.to_string())?;
                            }
                            1 => {
                                let state = store.load().map_err(|e| e.to_string())?;
                                let path = sdl_grabber::paths::checked_path(
                                    &state.config.destination,
                                    std::path::Path::new("report.jsonl"),
                                )
                                .map_err(|e| e.to_string())?;
                                craft_atomic::atomic_write_with(&path, |writer| {
                                    sdl_grabber::report::write_report(&state, writer)
                                })
                                .map_err(|e| e.to_string())?;
                                open::that(path).map_err(|e| e.to_string())?;
                            }
                            _ => {
                                store.delete_metadata().map_err(|e| e.to_string())?;
                                let _ = out.send(Message::Removed(id));
                            }
                        }
                        Ok::<_, String>(())
                    })();
                    if let Err(error) = result {
                        let _ = out.send(Message::Error(error));
                    }
                    repaint.request_repaint();
                });
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn site_grabber_nonempty_base_uses_safe_project_subfolder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.txt"), "keep me").unwrap();
        let mut draft = Wizard {
            name: "skk".into(),
            seed: "https://example.test/videos/".into(),
            destination: dir.path().to_string_lossy().into(),
            step: WizardStep::Destination,
            ..Default::default()
        };
        assert_eq!(draft.config().unwrap().destination, dir.path().join("skk"));
        draft.project_folder = "../outside".into();
        assert!(draft.destination_path().is_err());
        for reserved in ["CON", "NUL.txt", "COM¹", "LPT²", "CONIN$", "CON .txt"] {
            draft.project_folder = reserved.into();
            assert!(
                draft.destination_path().is_err(),
                "Reserved folder was accepted: {reserved}"
            );
        }
        draft.project_folder.clear();
        draft.name = "a/b:video".into();
        assert_eq!(
            draft.destination_path().unwrap(),
            dir.path().join("a_b_video")
        );
        draft.project_subfolder = false;
        assert_eq!(draft.destination_path().unwrap(), dir.path());
        #[cfg(windows)]
        {
            let drive = Wizard {
                name: "skk".into(),
                destination: "E:\\".into(),
                ..Default::default()
            };
            assert_eq!(drive.destination_path().unwrap(), PathBuf::from("E:\\skk"));
        }
    }
    #[test]
    fn site_grabber_stale_destination_check_cannot_advance_changed_input() {
        let mut draft = Wizard {
            name: "skk".into(),
            seed: "https://example.test/".into(),
            destination: "first".into(),
            step: WizardStep::Destination,
            ..Default::default()
        };
        let checked = draft.destination_path().unwrap();
        draft.destination = "second".into();
        assert!(draft.accept_destination_check(&checked, Ok(())).is_err());
        assert_eq!(draft.step, WizardStep::Destination);
        let checked = draft.destination_path().unwrap();
        assert!(
            draft
                .accept_destination_check(&checked, Err("Folder is not empty".into()))
                .is_err()
        );
        assert_eq!(draft.step, WizardStep::Destination);
        draft.accept_destination_check(&checked, Ok(())).unwrap();
        assert_eq!(draft.step, WizardStep::Explore);
    }
    #[test]
    fn site_grabber_startup_error_is_failed_and_empty_project_can_relocate() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        std::fs::create_dir(&old).unwrap();
        std::fs::write(old.join("existing.txt"), "keep me").unwrap();
        let store = ProjectStore::open(
            &dir.path().join("metadata"),
            &ProjectId::parse("skk").unwrap(),
        )
        .unwrap();
        let mut project = ProjectView::fixture();
        project.state.config.name = "skk".into();
        project.state.config.destination = old.clone();
        project.state.config.extensions.insert("mkv".into());
        project.state.next_run = Some(0);
        project.apply_result(Err("Choose an empty capture destination".into()));
        assert_eq!(project.state.status, ProjectStatus::Failed);
        assert!(
            project.state.next_run.is_none(),
            "Startup failures must not trigger a scheduled retry loop"
        );
        store.checkpoint(&project.state).unwrap();
        let base = dir.path().join("new");
        std::fs::create_dir(&base).unwrap();
        let relocated = relocate_uncaptured(&store, &base).unwrap();
        assert_eq!(relocated.config.destination, base.join("skk"));
        assert_eq!(relocated.status, ProjectStatus::Draft);
        assert!(relocated.config.extensions.contains("mkv"));
        assert_eq!(relocated.pending.len(), 1);
        assert_eq!(
            std::fs::read_to_string(old.join("existing.txt")).unwrap(),
            "keep me"
        );
        let mut partial = relocated;
        partial.consumed_bytes = 1;
        store.checkpoint(&partial).unwrap();
        let previous = std::fs::read(store.path()).unwrap();
        assert!(relocate_uncaptured(&store, &old).is_err());
        assert_eq!(std::fs::read(store.path()).unwrap(), previous);
    }
    #[test]
    fn site_grabber_wizard_validates_each_step_and_keeps_edits_on_back() {
        let mut draft = Wizard::default();
        assert!(draft.advance().is_err());
        assert_eq!(draft.step, WizardStep::Project);
        draft.name = "Images".into();
        draft.seed = "https://example.test/photos/".into();
        draft.advance().unwrap();
        assert_eq!(draft.step, WizardStep::Destination);
        assert!(draft.advance().is_err());
        draft.destination = "capture".into();
        draft.advance().unwrap();
        assert_eq!(draft.step, WizardStep::Explore);
        draft.depth = 6;
        draft.back();
        assert_eq!(draft.step, WizardStep::Destination);
        assert_eq!(draft.depth, 6);
        assert_eq!(draft.seed, "https://example.test/photos/");
    }
    #[test]
    fn site_grabber_image_template_and_file_size_map_to_capture_config() {
        let mut draft = Wizard {
            name: "Photos".into(),
            seed: "https://example.test/".into(),
            destination: "capture".into(),
            ..Default::default()
        };
        draft.apply_template(ProjectTemplate::Images);
        draft.resource_mib = 32;
        let config = draft.config().unwrap();
        for extension in ["jpg", "jpeg", "png", "gif", "webp"] {
            assert!(config.extensions.contains(extension));
        }
        assert_eq!(config.limits.resource_bytes, 32 << 20);
        draft.apply_template(ProjectTemplate::OfflineSite);
        assert!(draft.config().unwrap().extensions.is_empty());
    }
    #[test]
    fn site_grabber_invalid_draft_cannot_create() {
        let mut draft = Wizard::default();
        assert!(draft.config().is_err());
        draft.name = "Docs".into();
        draft.seed = "file:///private".into();
        draft.destination = "capture".into();
        assert!(draft.config().is_err());
        draft.seed = "https://example.test/docs/".into();
        assert!(draft.config().is_ok());
        draft.urls = 100_001;
        assert!(draft.config().is_err());
    }
    #[test]
    fn site_grabber_protected_storage_failure_has_no_plaintext_fallback() {
        struct Missing;
        impl crate::secrets::Store for Missing {
            fn probe(&self, _: &str) -> Result<(), crate::secrets::Fault> {
                Err(crate::secrets::Fault::Locked)
            }
            fn read(&self, _: &str, _: &str) -> Result<Option<String>, crate::secrets::Fault> {
                Err(crate::secrets::Fault::Locked)
            }
            fn write(
                &self,
                _: &str,
                _: &str,
                _: &str,
                _: &str,
            ) -> Result<(), crate::secrets::Fault> {
                panic!("A locked vault must not be written");
            }
            fn delete(&self, _: &str, _: &str) -> Result<(), crate::secrets::Fault> {
                Ok(())
            }
            fn list(&self, _: &str) -> Result<Vec<String>, crate::secrets::Fault> {
                Ok(Vec::new())
            }
        }
        assert!(save_credential(&Missing, "docs", "Bearer fixture-secret").is_err());
    }
    #[test]
    fn site_grabber_pause_stays_pending_until_durable_result() {
        let mut project = ProjectView::fixture();
        project.state.status = ProjectStatus::Running;
        project.pause_pending = true;
        assert_eq!(project.state.status, ProjectStatus::Running);
        let mut state = project.state.clone();
        state.status = ProjectStatus::Paused;
        project.apply_result(Ok(state));
        assert!(!project.pause_pending);
        assert_eq!(project.state.status, ProjectStatus::Paused);
    }
    #[test]
    fn site_grabber_repeated_schedule_ticks_do_not_start_paused_project() {
        let mut p = ProjectView::fixture();
        p.state.status = ProjectStatus::Paused;
        p.state.next_run = Some(0);
        assert!(!sdl_grabber::sync::due_run(&p.state, SystemTime::now()));
        p.state.status = ProjectStatus::Running;
        assert!(!sdl_grabber::sync::due_run(&p.state, SystemTime::now()));
    }
    #[test]
    fn site_grabber_calendar_reuses_weekday_and_once_rules() {
        let now = chrono::Local
            .with_ymd_and_hms(2026, 11, 24, 12, 0, 0)
            .single()
            .unwrap();
        let schedule = CalendarSchedule {
            hour: 13,
            minute: 0,
            days: [true; 7],
            once: Some((2026, 11, 25)),
        };
        let next = next_schedule(&schedule, now).unwrap();
        let next = chrono::Local
            .timestamp_opt(next as i64, 0)
            .single()
            .unwrap();
        assert_eq!((next.year(), next.month(), next.day()), (2026, 11, 25));
        assert!(next_schedule(&schedule, next).is_none());
    }

    #[test]
    fn site_grabber_one_time_schedule_can_be_more_than_a_year_away() {
        let now = chrono::Local
            .with_ymd_and_hms(2026, 11, 24, 12, 0, 0)
            .single()
            .unwrap();
        let schedule = CalendarSchedule {
            hour: 13,
            minute: 0,
            days: [true; 7],
            once: Some((2028, 11, 25)),
        };
        assert!(next_schedule(&schedule, now).is_some());
    }
    #[test]
    fn site_grabber_failed_empty_run_restarts_but_pause_preserves_accounting() {
        let mut state = ProjectView::fixture().state;
        state.pending.clear();
        state.status = ProjectStatus::Failed;
        assert!(should_synchronize(&state, false));
        state.status = ProjectStatus::Paused;
        assert!(!should_synchronize(&state, false));
        assert!(should_synchronize(&state, true));
    }

    #[tokio::test]
    async fn site_grabber_start_cannot_run_two_projects() {
        let (tx, rx) = mpsc::channel();
        let (commands, _commands) = tokio::sync::mpsc::channel(2);
        let (_events, events) = tokio::sync::mpsc::channel(2);
        let active_id = ProjectId::parse("active").unwrap();
        let mut manager = SiteGrabber {
            open: false,
            projects: vec![ProjectView::fixture()],
            wizard: Wizard::default(),
            creating: false,
            folder_pending: false,
            validating_destination: false,
            saving: false,
            loaded: true,
            selected: None,
            error: None,
            active: Some(Active {
                id: active_id.clone(),
                commands,
                events,
                saved: 0,
                pending: 1,
            }),
            tx,
            rx,
            session: String::new(),
            memory: BTreeMap::new(),
        };
        manager.dispatch(
            SiteAction::Start(ProjectId::parse("docs").unwrap()),
            &tokio::runtime::Handle::current(),
            Default::default(),
            &egui::Context::default(),
        );
        assert_eq!(manager.active.as_ref().unwrap().id, active_id);
        assert_eq!(manager.projects[0].state.status, ProjectStatus::Draft);
        assert!(manager.error.is_some());
    }
}
