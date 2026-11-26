//! Native file exploration, durable selection and guarded queue handoff.
use super::*;
use sdl_grabber::{
    ProjectMode, UrlKey,
    catalog::{CatalogStore, ExploreStatus, FileProject, ScanPending, ScheduledAction},
};
use std::collections::BTreeSet;
use tokio_util::sync::CancellationToken;

enum FileMessage {
    Loaded(Vec<(ProjectId, FileProject)>, Vec<String>),
    Saved(ProjectId, Result<FileProject, String>),
    Snapshot(ProjectId, FileProject),
    Finished(ProjectId, Result<FileProject, String>),
    Prepared(ProjectId, Result<(FileProject, Vec<u64>), String>, bool),
    Folder(Option<PathBuf>),
}
pub(super) struct FilesWorkspace {
    projects: BTreeMap<ProjectId, FileProject>,
    selected: Option<ProjectId>,
    wizard: Option<Wizard>,
    editing: Option<ProjectId>,
    active: Option<(ProjectId, CancellationToken)>,
    busy: bool,
    loaded: bool,
    pub(super) error: Option<String>,
    filter: String,
    source_filter: Option<String>,
    sort_size: bool,
    imports: String,
    session: String,
    memory: BTreeMap<SessionKey, String>,
    scheduled_download: Option<ProjectId>,
    browser_links: Vec<crate::transfer::Imported>,
    browser_cookie: Option<(String, String)>,
    transfer_bytes: BTreeMap<ProjectId, u64>,
    tx: mpsc::Sender<FileMessage>,
    rx: mpsc::Receiver<FileMessage>,
}
type SessionKey = (ProjectId, String, String);
fn session_key(id: &ProjectId, reference: &CredentialRef) -> SessionKey {
    (
        id.clone(),
        reference.identifier.clone(),
        reference.origin.clone(),
    )
}
fn browser_reference(origin: &str, project: &FileProject) -> Result<CredentialRef, String> {
    if origin != project.config.seed.origin().ascii_serialization() {
        return Err(t(
            "The browser session belongs to another origin. Import again or supply a new session.",
        )
        .into());
    }
    Ok(CredentialRef {
        identifier: "memory-cookie".into(),
        origin: origin.into(),
    })
}
fn protected_reference(id: &ProjectId, project: &FileProject) -> CredentialRef {
    let hash = UrlKey::of(&project.config.seed);
    CredentialRef {
        identifier: format!(
            "{}-session-{}-{}",
            id.as_str(),
            project.scan_revision,
            &hash.0[..12]
        ),
        origin: project.config.seed.origin().ascii_serialization(),
    }
}
fn catalog(id: &ProjectId) -> Result<CatalogStore, String> {
    CatalogStore::open(&root()?, id).map_err(|e| e.to_string())
}
fn default_wizard() -> Wizard {
    Wizard {
        mode: ProjectMode::Files,
        template: ProjectTemplate::Custom,
        budget_mib: 64,
        ..Default::default()
    }
}
fn file_rules(w: &Wizard) -> Result<sdl_grabber::catalog::FileRules, String> {
    let size = |text: &str| -> Result<Option<u64>, String> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        text.trim()
            .parse::<u64>()
            .ok()
            .and_then(|n| n.checked_mul(1 << 20))
            .map(Some)
            .ok_or_else(|| t("Invalid file size").into())
    };
    let rules = sdl_grabber::catalog::FileRules {
        include: w
            .file_includes
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        exclude: w
            .file_excludes
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        minimum_bytes: size(&w.minimum_mib)?,
        maximum_bytes: size(&w.maximum_mib)?,
    };
    rules.validate().map_err(|e| e.to_string())?;
    Ok(rules)
}
pub(super) fn is_file_action(action: &SiteAction) -> bool {
    matches!(
        action,
        SiteAction::StartExploring(_)
            | SiteAction::StopExploring(_)
            | SiteAction::CheckCandidates(..)
            | SiteAction::DownloadSelected(_)
            | SiteAction::AddSelectedToQueue(_)
            | SiteAction::SaveFileProject(..)
            | SiteAction::EditFileProject(_)
            | SiteAction::ImportFileLinks(..)
            | SiteAction::FileJobsReady(..)
            | SiteAction::OpenCandidateMedia(..)
            | SiteAction::BrowseFilesDestination
    )
}
impl Drop for FilesWorkspace {
    fn drop(&mut self) {
        if let Some((_, cancel)) = &self.active {
            cancel.cancel();
        }
    }
}
impl FilesWorkspace {
    pub(super) fn new(rt: &tokio::runtime::Handle) -> Self {
        let (tx, rx) = mpsc::channel();
        let out = tx.clone();
        rt.spawn_blocking(move || {
            let mut projects = Vec::new();
            let mut errors = Vec::new();
            if let Ok(root) = root()
                && let Ok(entries) = std::fs::read_dir(root.join("file-projects-v1"))
            {
                for entry in entries.flatten() {
                    if let Ok(id) = ProjectId::parse(&entry.file_name().to_string_lossy()) {
                        match catalog(&id).and_then(|s| s.load().map_err(|e| e.to_string())) {
                            Ok(p) => projects.push((id, p)),
                            Err(e) => errors.push(e),
                        }
                    }
                }
            }
            let _ = out.send(FileMessage::Loaded(projects, errors));
        });
        Self {
            projects: BTreeMap::new(),
            selected: None,
            wizard: None,
            editing: None,
            active: None,
            busy: false,
            loaded: false,
            error: None,
            filter: String::new(),
            source_filter: None,
            sort_size: false,
            imports: String::new(),
            session: String::new(),
            memory: BTreeMap::new(),
            scheduled_download: None,
            browser_links: Vec::new(),
            browser_cookie: None,
            transfer_bytes: BTreeMap::new(),
            tx,
            rx,
        }
    }
    pub(super) fn preview(&mut self, stage: &str) {
        let mut w = default_wizard();
        w.name = "Series files".into();
        w.seed = "https://example.test/series/".into();
        w.destination = "C:\\Downloads".into();
        w.step = match stage {
            "files-project" => WizardStep::Project,
            "files-destination" => WizardStep::Destination,
            "files-explore" => WizardStep::Explore,
            "files-filters" => WizardStep::Filters,
            _ => WizardStep::Project,
        };
        if stage != "files-results" {
            self.wizard = Some(w);
            return;
        }
        let mut p = FileProject::new(w.config().unwrap());
        p.explore_status = ExploreStatus::Completed;
        for (name, size) in [
            ("episode-01.mkv", Some(1048576)),
            ("episode-02.mp4", Some(2097152)),
            ("episode-03.mkv", None),
        ] {
            let url = p.config.seed.join(&format!("season-01/{name}")).unwrap();
            let key = UrlKey::of(&url);
            p.merge(sdl_grabber::CandidateMetadata {
                url: url.clone(),
                final_url: url,
                name: name.into(),
                mime: Some("video/mp4".into()),
                size,
                source_page: p.config.seed.join("season-01/").unwrap(),
            });
            p.candidates.get_mut(&key).unwrap().checked = size.is_some();
        }
        let id = ProjectId::parse("preview-files").unwrap();
        self.projects.insert(id.clone(), p);
        self.selected = Some(id);
        self.wizard = None;
        self.loaded = true;
    }
    pub(super) fn import_browser(&mut self, command: &sdl_ipc::Command) {
        self.browser_links.clear();
        self.browser_cookie = None;
        if command.proxy.is_some() || command.proxy_pac.is_some() || command.proxy_login {
            self.error=Some(t("Site Grabber cannot verify the browser's proxy route. Use the browser download or media tools.").into());
            return;
        }
        let Some(page) = command
            .referer
            .as_ref()
            .and_then(|r| Url::parse(r).ok())
            .filter(|u| sdl_grabber::normalize_url(u, u.as_str()).is_ok())
        else {
            self.error = Some(t("Browser import requires a valid source page").into());
            return;
        };
        let mut wizard = default_wizard();
        wizard.name = page.host_str().unwrap_or("Website").into();
        wizard.seed = page.to_string();
        wizard.destination = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("Downloads"))
            .to_string_lossy()
            .into_owned();
        self.browser_links = command
            .links
            .iter()
            .map(|url| crate::transfer::Imported {
                url: url.clone(),
                referer: Some(page.to_string()),
                ..Default::default()
            })
            .collect();
        self.browser_cookie = command
            .cookies
            .clone()
            .map(|cookie| (page.origin().ascii_serialization(), cookie));
        self.wizard = Some(wizard);
        self.editing = None;
    }
    pub(super) fn session_for(
        &self,
        id: &ProjectId,
        reference: &CredentialRef,
    ) -> Result<String, String> {
        resolve_session(
            reference,
            self.memory.get(&session_key(id, reference)).cloned(),
        )
    }
    pub(super) fn selected_count(&self, id: &ProjectId) -> usize {
        self.projects.get(id).map_or(0, |p| {
            p.candidates
                .values()
                .filter(|r| r.checked && r.dispatch_id.is_none())
                .count()
        })
    }
    pub(super) fn candidate(
        &self,
        id: &ProjectId,
        key: &UrlKey,
    ) -> Option<sdl_grabber::CandidateMetadata> {
        self.projects
            .get(id)?
            .candidates
            .get(key)
            .map(|r| r.metadata.clone())
    }
    pub(super) fn sync_items(&mut self, items: &[crate::model::Item]) {
        self.transfer_bytes.clear();
        for item in items {
            if let Some(site) = &item.site {
                let bytes = self
                    .transfer_bytes
                    .entry(site.project_id.clone())
                    .or_default();
                *bytes = bytes.saturating_add(item.downloaded);
            }
        }
        for item in items {
            if let Some(site) = &item.site
                && let Some(p) = self.projects.get_mut(&site.project_id)
                && let Some(r) = p.candidates.get_mut(&site.candidate_id)
            {
                r.item_id = Some(item.id);
                r.status = match item.status {
                    crate::model::Status::Queued | crate::model::Status::Stopped => {
                        sdl_grabber::catalog::TransferStatus::Queued
                    }
                    crate::model::Status::Downloading => {
                        sdl_grabber::catalog::TransferStatus::Downloading
                    }
                    crate::model::Status::Complete => {
                        sdl_grabber::catalog::TransferStatus::Completed
                    }
                    crate::model::Status::Error => sdl_grabber::catalog::TransferStatus::Failed,
                };
                r.reason = item.error.clone();
            }
        }
    }
    pub(super) fn seed(&self, action: &SiteAction) -> Option<Url> {
        let id = match action {
            SiteAction::StartExploring(id)
            | SiteAction::DownloadSelected(id)
            | SiteAction::AddSelectedToQueue(id) => id,
            _ => return None,
        };
        self.projects.get(id).map(|p| p.config.seed.clone())
    }
    pub(super) fn poll(&mut self, ctx: &egui::Context) -> Option<SiteAction> {
        let mut next = None;
        for _ in 0..64 {
            let Ok(message) = self.rx.try_recv() else {
                break;
            };
            match message {
                FileMessage::Loaded(projects, errors) => {
                    self.projects.extend(projects);
                    self.loaded = true;
                    if !errors.is_empty() {
                        self.error = Some(errors.join("; "));
                    }
                }
                FileMessage::Saved(id, result) => {
                    self.busy = false;
                    match result {
                        Ok(p) => {
                            self.projects.insert(id.clone(), p);
                            self.selected = Some(id);
                            self.wizard = None;
                            self.editing = None;
                            self.browser_links.clear();
                            self.browser_cookie = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                FileMessage::Snapshot(id, p) => {
                    self.projects.insert(id, p);
                }
                FileMessage::Finished(id, result) => {
                    self.active = None;
                    match result {
                        Ok(p) => {
                            let auto = (p.auto_download
                                || self.scheduled_download.as_ref() == Some(&id))
                                && !matches!(
                                    p.explore_status,
                                    ExploreStatus::Failed | ExploreStatus::Stopped
                                );
                            self.scheduled_download = None;
                            self.projects.insert(id.clone(), p);
                            if auto {
                                next = Some(SiteAction::DownloadSelected(id));
                            }
                        }
                        Err(e) => {
                            self.error = Some(e);
                            if let Some(p) = self.projects.get_mut(&id) {
                                p.explore_status = ExploreStatus::Failed;
                                p.next_run = None;
                            }
                        }
                    }
                }
                FileMessage::Prepared(id, result, start) => {
                    self.busy = false;
                    match result {
                        Ok((p, ids)) => {
                            self.projects.insert(id, p);
                            next = Some(SiteAction::FileJobsReady(ids, start));
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                FileMessage::Folder(path) => {
                    self.busy = false;
                    if let (Some(w), Some(path)) = (&mut self.wizard, path) {
                        w.destination = path.to_string_lossy().into_owned();
                    }
                }
            }
            ctx.request_repaint();
        }
        if next.is_none() && self.active.is_none() && !self.busy && self.loaded {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if let Some((id, p)) = self.projects.iter_mut().find(|(_, p)| {
                !matches!(
                    p.explore_status,
                    ExploreStatus::Stopped | ExploreStatus::Failed
                ) && p.next_run.is_some_and(|at| at <= now)
            }) {
                p.next_run = p
                    .schedule
                    .as_ref()
                    .and_then(|s| next_schedule(s, chrono::Local::now()));
                next = Some(match p.scheduled_action {
                    ScheduledAction::DownloadChecked => SiteAction::DownloadSelected(id.clone()),
                    _ => {
                        if matches!(
                            p.scheduled_action,
                            ScheduledAction::ExploreAndDownload | ScheduledAction::Synchronize
                        ) {
                            self.scheduled_download = Some(id.clone());
                            p.check_all(true);
                        }
                        p.rescan();
                        SiteAction::StartExploring(id.clone())
                    }
                });
            }
        }
        if self.active.is_some() || self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        next
    }
    pub(super) fn show(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        files_mode: &mut bool,
    ) -> Outcome<SiteAction> {
        let mut action = None;
        let outcome = crate::dialogs::window(
            ctx,
            "site-grabber",
            "Site Grabber",
            [1000.0, 720.0],
            true,
            crate::theme::colors().window,
            |ui, origin| {
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_size(origin, egui::vec2(1000.0, 720.0)))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        ui.set_width(980.0);
                        ui.visuals_mut().override_text_color = Some(crate::theme::colors().text);
                        ui.horizontal(|ui| {
                            ui.heading(t("Site Grabber"));
                            ui.label(t("Files"));
                            if ui.button(t("Offline website")).clicked() {
                                *files_mode = false;
                            }
                            if ui.button(t("Close")).clicked() {
                                *open = false;
                            }
                        });
                        if let Some(error) = &self.error {
                            ui.colored_label(crate::theme::colors().muted, error);
                        }
                        if self.wizard.is_some() {
                            self.wizard_view(ui, &mut action);
                        } else {
                            self.results_view(ui, &mut action);
                        }
                    },
                );
                Outcome::<()>::Open
            },
        );
        if matches!(outcome, Outcome::Closed) {
            *open = false;
        }
        if !*open {
            self.browser_links.clear();
            self.browser_cookie = None;
            if let Some(w) = &mut self.wizard {
                w.authorization.clear();
            }
        }
        match action {
            Some(a) => Outcome::Done(a),
            None if !*open => Outcome::Closed,
            None => Outcome::Open,
        }
    }
    fn wizard_view(&mut self, ui: &mut egui::Ui, action: &mut Option<SiteAction>) {
        let w = self.wizard.as_mut().unwrap();
        ui.horizontal(|ui| {
            for step in WizardStep::ALL {
                ui.add_enabled(
                    false,
                    egui::Button::new(step.title()).selected(w.step == step),
                );
            }
        });
        egui::ScrollArea::vertical()
            .max_height(560.0)
            .show(ui, |ui| {
                if w.fields(ui, self.busy) {
                    *action = Some(SiteAction::BrowseFilesDestination);
                }
            });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new(t("Back")))
                .clicked()
            {
                w.back();
            }
            if w.step != WizardStep::Filters {
                if ui
                    .add_enabled(!self.busy, egui::Button::new(t("Next")))
                    .clicked()
                {
                    self.error = w.advance().err();
                }
            } else if ui
                .add_enabled(!self.busy, egui::Button::new(t("Save project")))
                .clicked()
            {
                let result = (|| {
                    let config = w.config()?;
                    let rules = file_rules(w)?;
                    let id = self.editing.clone().unwrap_or_else(|| {
                        ProjectId::parse(&format!(
                            "files-{}",
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos()
                        ))
                        .unwrap()
                    });
                    let mut p = self
                        .projects
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| FileProject::new(config.clone()));
                    p.config = config;
                    p.rules = rules;
                    p.layout = w.layout;
                    p.collision = w.collision;
                    p.auto_download = w.auto_download;
                    p.schedule = w.schedule()?;
                    p.scheduled_action = w.scheduled_action;
                    p.rescan();
                    p.next_run = p
                        .schedule
                        .as_ref()
                        .and_then(|s| next_schedule(s, chrono::Local::now()));
                    for link in &self.browser_links {
                        if let Ok(url) = Url::parse(&link.url)
                            && sdl_grabber::normalize_url(&url, url.as_str()).is_ok()
                            && p.frontier.len() < p.config.limits.urls
                        {
                            p.frontier.push_back(ScanPending {
                                url,
                                depth: 0,
                                source_page: link
                                    .referer
                                    .as_ref()
                                    .and_then(|r| Url::parse(r).ok())
                                    .unwrap_or_else(|| p.config.seed.clone()),
                            });
                        }
                    }
                    if let Some((origin, cookie)) = &self.browser_cookie {
                        let reference = browser_reference(origin, &p)?;
                        self.memory
                            .insert(session_key(&id, &reference), cookie.clone());
                        p.credential = Some(reference);
                    }
                    Ok::<_, String>((id, p))
                })();
                match result {
                    Ok((id, p)) => *action = Some(SiteAction::SaveFileProject(id, Box::new(p))),
                    Err(e) => self.error = Some(e),
                }
            }
        });
    }
    fn results_view(&mut self, ui: &mut egui::Ui, action: &mut Option<SiteAction>) {
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new(t("New project")))
                .clicked()
            {
                self.browser_links.clear();
                self.browser_cookie = None;
                self.wizard = Some(default_wizard());
                self.editing = None;
                self.error = None;
            }
            let label = self
                .selected
                .as_ref()
                .and_then(|id| self.projects.get(id))
                .map_or(t("Saved projects"), |p| p.config.name.as_str());
            egui::ComboBox::from_id_salt("file-projects")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (id, p) in &self.projects {
                        ui.selectable_value(&mut self.selected, Some(id.clone()), &p.config.name);
                    }
                });
            if !self.loaded {
                ui.spinner();
            }
        });
        let Some((id, p)) = self
            .selected
            .as_ref()
            .and_then(|id| self.projects.get(id).map(|p| (id, p)))
        else {
            ui.label(t(
                "Explore a website, review matching files, then download checked files.",
            ));
            return;
        };
        ui.label(sdl_grabber::report::sanitized_url(&p.config.seed));
        ui.label(p.config.destination.to_string_lossy());
        let (count, bytes, unknown) = p.selected_totals();
        ui.horizontal_wrapped(|ui| {
            ui.label(t("Candidates"));
            ui.label(p.candidates.len().to_string());
            ui.label(t("Checked"));
            ui.label(count.to_string());
            ui.label(crate::model::size(bytes));
            ui.label(t("Unknown sizes"));
            ui.label(unknown.to_string());
            ui.label(t("Scan bytes"));
            ui.label(crate::model::size(p.scan_bytes));
            ui.label(explore_label(p.explore_status));
        });
        ui.horizontal_wrapped(|ui| {
            for (status, label) in [
                (sdl_grabber::catalog::TransferStatus::Queued, t("Queued")),
                (
                    sdl_grabber::catalog::TransferStatus::Downloading,
                    t("Downloading"),
                ),
                (
                    sdl_grabber::catalog::TransferStatus::Completed,
                    t("Completed"),
                ),
                (sdl_grabber::catalog::TransferStatus::Failed, t("Failed")),
            ] {
                ui.label(label);
                ui.label(
                    p.candidates
                        .values()
                        .filter(|r| r.status == status)
                        .count()
                        .to_string(),
                );
            }
            ui.label(t("Transfer bytes"));
            ui.label(crate::model::size(
                self.transfer_bytes.get(id).copied().unwrap_or(0),
            ));
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy && self.active.is_none(),
                    egui::Button::new(t("Start Exploring")),
                )
                .clicked()
            {
                *action = Some(SiteAction::StartExploring(id.clone()));
            }
            if ui
                .add_enabled(
                    self.active.as_ref().is_some_and(|(active, _)| active == id),
                    egui::Button::new(t("Stop Exploring")),
                )
                .clicked()
            {
                *action = Some(SiteAction::StopExploring(id.clone()));
            }
            if ui
                .add_enabled(
                    !self.busy && self.active.is_none(),
                    egui::Button::new(t("Check All")),
                )
                .clicked()
            {
                *action = Some(SiteAction::CheckCandidates(
                    id.clone(),
                    p.candidates.keys().map(|k| (k.clone(), true)).collect(),
                ));
            }
            if ui
                .add_enabled(
                    !self.busy && self.active.is_none(),
                    egui::Button::new(t("Uncheck All")),
                )
                .clicked()
            {
                *action = Some(SiteAction::CheckCandidates(
                    id.clone(),
                    p.candidates.keys().map(|k| (k.clone(), false)).collect(),
                ));
            }
            if ui
                .add_enabled(
                    count > 0 && !self.busy,
                    egui::Button::new(t("Download checked")),
                )
                .clicked()
            {
                *action = Some(SiteAction::DownloadSelected(id.clone()));
            }
            if ui
                .add_enabled(
                    count > 0 && !self.busy,
                    egui::Button::new(t("Add to queue")),
                )
                .clicked()
            {
                *action = Some(SiteAction::AddSelectedToQueue(id.clone()));
            }
            if ui
                .add_enabled(
                    !self.busy && self.active.is_none(),
                    egui::Button::new(t("Edit project")),
                )
                .clicked()
            {
                *action = Some(SiteAction::EditFileProject(id.clone()));
            }
        });
        ui.horizontal(|ui| {
            let l = ui.label(t("Filter results"));
            ui.text_edit_singleline(&mut self.filter).labelled_by(l.id);
            ui.checkbox(&mut self.sort_size, t("Sort by size"));
        });
        // Tree nodes are source directories. Candidate identity and selection are independent of view order.
        let sources: BTreeSet<_> = p
            .candidates
            .values()
            .map(|r| sdl_grabber::report::sanitized_url(&r.metadata.source_page))
            .collect();
        let mut rows: Vec<_> = p
            .candidates
            .iter()
            .filter(|(_, r)| {
                self.filter.is_empty()
                    || r.metadata
                        .name
                        .to_lowercase()
                        .contains(&self.filter.to_lowercase())
            })
            .filter(|(_, r)| {
                self.source_filter.as_ref().is_none_or(|s| {
                    s == &sdl_grabber::report::sanitized_url(&r.metadata.source_page)
                })
            })
            .collect();
        if self.sort_size {
            rows.sort_by_key(|(_, r)| r.metadata.size);
        } else {
            rows.sort_by(|a, b| a.1.metadata.name.cmp(&b.1.metadata.name));
        }
        ui.columns(2, |columns| {
            egui::ScrollArea::vertical()
                .id_salt("file-tree")
                .max_height(340.0)
                .show(&mut columns[0], |ui| {
                    ui.selectable_value(&mut self.source_filter, None, t("All source folders"));
                    for source in sources {
                        ui.selectable_value(&mut self.source_filter, Some(source.clone()), source);
                    }
                });
            egui::ScrollArea::vertical()
                .id_salt("file-results")
                .max_height(340.0)
                .show_rows(&mut columns[1], 56.0, rows.len(), |ui, range| {
                    for index in range {
                        let (key, r) = rows[index];
                        ui.push_id(&key.0, |ui| {
                            ui.horizontal(|ui| {
                                let mut checked = r.checked;
                                let checkbox = ui.add_enabled(
                                    !self.busy
                                        && self.active.is_none()
                                        && (r.dispatch_id.is_some()
                                            || p.download_allowed(&r.metadata)),
                                    egui::Checkbox::new(&mut checked, ""),
                                );
                                let width = (ui.available_width() - 160.0).max(60.0);
                                let name = ui
                                    .add_sized(
                                        [width, 22.0],
                                        egui::Label::new(&r.metadata.name).truncate(),
                                    )
                                    .on_hover_text(&r.metadata.name);
                                if checkbox.labelled_by(name.id).changed() {
                                    *action = Some(SiteAction::CheckCandidates(
                                        id.clone(),
                                        vec![(key.clone(), checked)],
                                    ));
                                }
                                ui.label(
                                    r.metadata.size.map_or_else(
                                        || t("Unknown").to_owned(),
                                        crate::model::size,
                                    ),
                                );
                                ui.label(transfer_label(r.status));
                            });
                            let details = format!(
                                "{}\n{}\n{}",
                                sdl_grabber::report::sanitized_url(&r.metadata.final_url),
                                sdl_grabber::report::sanitized_url(&r.metadata.source_page),
                                r.reason.as_deref().unwrap_or("")
                            );
                            if sdl_grabber::classify::is_manifest(&r.metadata) {
                                if ui
                                    .button(t("Open media tools"))
                                    .on_hover_text(details)
                                    .clicked()
                                {
                                    *action = Some(SiteAction::OpenCandidateMedia(
                                        id.clone(),
                                        key.clone(),
                                    ));
                                }
                            } else {
                                ui.add(
                                    egui::Label::new(
                                        r.metadata.mime.as_deref().unwrap_or(t("Unknown")),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(details);
                            }
                        });
                    }
                });
        });
        if p.credential.is_some() {
            ui.horizontal(|ui| {
                let l = ui.label(t("Replacement session"));
                ui.add(egui::TextEdit::singleline(&mut self.session).password(true))
                    .labelled_by(l.id);
                if ui.button(t("Use session")).clicked()
                    && let Some(reference) = &p.credential
                {
                    self.memory.insert(
                        session_key(id, reference),
                        std::mem::take(&mut self.session),
                    );
                }
            });
        }
        ui.collapsing(t("Import browser links"),|ui|{
            ui.label(t("Paste links exported from your browser. Imported links keep their source page and are checked before use."));
            ui.text_edit_multiline(&mut self.imports);
            if ui.add_enabled(!self.busy && self.active.is_none(),egui::Button::new(t("Import links"))).clicked(){*action=Some(SiteAction::ImportFileLinks(id.clone(),std::mem::take(&mut self.imports)));}
        });
        ui.collapsing(t("Skipped URLs and limitations"),|ui|{
            ui.label(t("Static HTTP(S) discovery only. Dynamic pages need browser link import. HLS/DASH uses media tools. DRM, FTP traversal, proxy-side DNS and app-closed schedules are unsupported."));
            egui::ScrollArea::vertical().max_height(100.0).show(ui,|ui|{for r in p.reports.iter().rev().take(50){ui.label(format!("{}: {}",sdl_grabber::report::sanitized_url(&r.url),r.reason));}});
        });
    }
    pub(super) fn dispatch(
        &mut self,
        action: SiteAction,
        rt: &tokio::runtime::Handle,
        options: sdl_engine::Options,
        ctx: &egui::Context,
    ) {
        let out = self.tx.clone();
        let repaint = ctx.clone();
        match action {
            SiteAction::BrowseFilesDestination => {
                if self.busy {
                    return;
                }
                self.busy = true;
                rt.spawn_blocking(move || {
                    let _ = out.send(FileMessage::Folder(rfd::FileDialog::new().pick_folder()));
                    repaint.request_repaint();
                });
            }
            SiteAction::EditFileProject(id) => {
                if self.active.is_some() || self.busy {
                    return;
                }
                if let Some(p) = self.projects.get(&id) {
                    self.wizard = Some(wizard_from(p));
                    self.editing = Some(id);
                }
            }
            SiteAction::SaveFileProject(id, mut p) => {
                if self.busy {
                    return;
                }
                self.busy = true;
                let auth = self
                    .wizard
                    .as_mut()
                    .filter(|w| w.authenticated)
                    .map(|w| std::mem::take(&mut w.authorization));
                let protect = self.wizard.as_ref().is_some_and(|w| w.protect);
                if let Some(value) = &auth
                    && !protect
                {
                    let reference = CredentialRef {
                        identifier: "memory-only".into(),
                        origin: p.config.seed.origin().ascii_serialization(),
                    };
                    self.memory
                        .insert(session_key(&id, &reference), value.clone());
                }
                rt.spawn_blocking(move || {
                    let result = (|| {
                        p.config.validate()?;
                        p.rules.validate().map_err(|e| e.to_string())?;
                        sdl_grabber::paths::checked_path(
                            &p.config.destination,
                            std::path::Path::new(""),
                        )
                        .map_err(|e| e.to_string())?;
                        if let Some(value) = auth {
                            let reference = if protect {
                                protected_reference(&id, &p)
                            } else {
                                CredentialRef {
                                    identifier: "memory-only".into(),
                                    origin: p.config.seed.origin().ascii_serialization(),
                                }
                            };
                            if protect {
                                save_credential(
                                    &crate::secrets::System,
                                    &reference.identifier,
                                    &value,
                                )?;
                            }
                            p.credential = Some(reference);
                        }
                        p.validate().map_err(|e| e.to_string())?;
                        catalog(&id)?.checkpoint(&p).map_err(|e| e.to_string())?;
                        Ok(*p)
                    })();
                    let _ = out.send(FileMessage::Saved(id, result));
                    repaint.request_repaint();
                });
            }
            SiteAction::CheckCandidates(id, changes) => {
                if self.busy || self.active.is_some() {
                    return;
                }
                if let Some(p) = self.projects.get_mut(&id) {
                    for (key, checked) in changes {
                        let allowed = p.candidates.get(&key).is_some_and(|r| {
                            r.dispatch_id.is_some() || p.download_allowed(&r.metadata)
                        });
                        if let Some(r) = p.candidates.get_mut(&key) {
                            r.checked = checked && allowed;
                        }
                    }
                    // Selection is merged into disk's current frontier, so a scan cannot erase it.
                    let checked: BTreeMap<_, _> = p
                        .candidates
                        .iter()
                        .map(|(k, r)| (k.clone(), r.checked))
                        .collect();
                    self.busy = true;
                    rt.spawn_blocking(move || {
                        let result = (|| {
                            let store = catalog(&id)?;
                            let mut p = store.load().map_err(|e| e.to_string())?;
                            for (k, value) in checked {
                                if let Some(r) = p.candidates.get_mut(&k) {
                                    r.checked = value;
                                }
                            }
                            store.checkpoint(&p).map_err(|e| e.to_string())?;
                            Ok(p)
                        })();
                        let _ = out.send(FileMessage::Saved(id, result));
                        repaint.request_repaint();
                    });
                }
            }
            SiteAction::StopExploring(id) => {
                if let Some((active, cancel)) = &self.active
                    && *active == id
                {
                    cancel.cancel();
                }
            }
            SiteAction::StartExploring(id) => {
                if self.active.is_some() || self.busy {
                    return;
                }
                let Some(mut p) = self.projects.get(&id).cloned() else {
                    return;
                };
                if p.frontier.is_empty() {
                    p.rescan();
                }
                let scheduled_download = self.scheduled_download.as_ref() == Some(&id);
                let cancel = CancellationToken::new();
                self.active = Some((id.clone(), cancel.clone()));
                let memory = p
                    .credential
                    .as_ref()
                    .and_then(|reference| self.memory.get(&session_key(&id, reference)).cloned());
                let handle = rt.clone();
                rt.spawn_blocking(move || {
                    let result = (|| {
                        let store = catalog(&id)?;
                        let network = if p.config.allow_private_seed {
                            sdl_engine::crawl_fetch::NetworkAccess::with_private_origin(
                                p.config.seed.origin().ascii_serialization(),
                            )
                        } else {
                            sdl_engine::crawl_fetch::NetworkAccess::public()
                        };
                        let mut transport = CrawlTransport::new(options, Arc::new(network));
                        if let Some(reference) = &p.credential {
                            if reference.origin != p.config.seed.origin().ascii_serialization() {
                                return Err(t("Session origin does not match the project").into());
                            }
                            let value = resolve_session(reference, memory)?;
                            let mut header = value
                                .parse::<reqwest::header::HeaderValue>()
                                .map_err(|_| t("Invalid authorization header").to_owned())?;
                            header.set_sensitive(true);
                            transport.headers.insert(
                                if reference.identifier == "memory-cookie" {
                                    reqwest::header::COOKIE
                                } else {
                                    reqwest::header::AUTHORIZATION
                                },
                                header,
                            );
                            transport.credential_origin = Some(reference.origin.clone());
                        }
                        let send = |snapshot: &FileProject| {
                            let _ = out.send(FileMessage::Snapshot(id.clone(), snapshot.clone()));
                            repaint.request_repaint();
                        };
                        handle
                            .block_on(sdl_grabber::explore::explore_catalog(
                                &mut p,
                                Some(&store),
                                transport,
                                cancel,
                                Some(&send),
                            ))
                            .map_err(|e| e.to_string())?;
                        if scheduled_download && p.explore_status != ExploreStatus::Stopped {
                            p.check_all(true);
                        }
                        if p.explore_status != ExploreStatus::Stopped {
                            p.next_run = p
                                .schedule
                                .as_ref()
                                .and_then(|s| next_schedule(s, chrono::Local::now()));
                            store.checkpoint(&p).map_err(|e| e.to_string())?;
                        }
                        Ok(p)
                    })();
                    if result.is_err()
                        && let Ok(store) = catalog(&id)
                        && let Ok(mut state) = store.load()
                    {
                        state.explore_status = ExploreStatus::Failed;
                        state.next_run = None;
                        let _ = store.checkpoint(&state);
                    }
                    let _ = out.send(FileMessage::Finished(id, result));
                    repaint.request_repaint();
                });
            }
            SiteAction::ImportFileLinks(id, text) => {
                if self.active.is_some() || self.busy {
                    return;
                }
                if let Some(mut p) = self.projects.get(&id).cloned() {
                    for link in crate::transfer::import(&text) {
                        if let Ok(url) = Url::parse(&link.url)
                            && sdl_grabber::normalize_url(&url, url.as_str()).is_ok()
                            && p.frontier.len() < p.config.limits.urls
                        {
                            let source_page = link
                                .referer
                                .and_then(|r| Url::parse(&r).ok())
                                .unwrap_or_else(|| p.config.seed.clone());
                            p.seen.remove(&UrlKey::of(&url));
                            p.frontier.push_back(ScanPending {
                                url,
                                depth: 0,
                                source_page,
                            });
                        }
                    }
                    self.busy = true;
                    rt.spawn_blocking(move || {
                        let result = catalog(&id)
                            .and_then(|s| s.checkpoint(&p).map_err(|e| e.to_string()))
                            .map(|()| p);
                        let _ = out.send(FileMessage::Saved(id, result));
                        repaint.request_repaint();
                    });
                }
            }
            _ => {}
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        id: ProjectId,
        start: bool,
        first_id: u64,
        reserved: BTreeSet<PathBuf>,
        rt: &tokio::runtime::Handle,
        options: sdl_engine::Options,
        ctx: &egui::Context,
    ) {
        if self.busy || self.active.is_some() {
            self.error =
                Some(t("Stop exploration before adding checked files to the queue").into());
            return;
        }
        let Some(mut p) = self.projects.get(&id).cloned() else {
            return;
        };
        self.busy = true;
        let out = self.tx.clone();
        let repaint = ctx.clone();
        rt.spawn_blocking(move || {
            let result = (|| {
                let store = catalog(&id)?;
                let ids = crate::site_grabber_jobs::prepare(
                    &id, &mut p, &store, &options, first_id, reserved,
                )
                .map_err(|e| e.to_string())?;
                Ok((p, ids))
            })();
            if result.is_err()
                && let Ok(store) = catalog(&id)
                && let Ok(mut state) = store.load()
            {
                state.explore_status = ExploreStatus::Failed;
                state.next_run = None;
                let _ = store.checkpoint(&state);
            }
            let _ = out.send(FileMessage::Prepared(id, result, start));
            repaint.request_repaint();
        });
    }
}
pub(super) fn resolve_session(
    reference: &CredentialRef,
    memory: Option<String>,
) -> Result<String, String> {
    match memory {
        Some(value) => Ok(value),
        None if !matches!(
            reference.identifier.as_str(),
            "memory-only" | "memory-cookie"
        ) =>
        {
            crate::secrets::Store::read(
                &crate::secrets::System,
                CREDENTIAL_SERVICE,
                &reference.identifier,
            )
            .ok()
            .flatten()
            .ok_or_else(|| t("Unlock the credential vault or enter a replacement session").into())
        }
        None => Err(t("Enter a replacement memory-only session before resuming").into()),
    }
}
fn wizard_from(p: &FileProject) -> Wizard {
    let mut w = default_wizard();
    w.name = p.config.name.clone();
    w.seed = p.config.seed.to_string();
    w.destination = p.config.destination.to_string_lossy().into_owned();
    w.project_subfolder = false;
    w.path_prefix = p.config.path_prefix.clone();
    w.page_origins = p
        .config
        .page_origins
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    w.asset_origins = p
        .config
        .resource_origins
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    w.includes = p.config.include_prefixes.join(" ");
    w.excludes = p.config.exclude_prefixes.join(" ");
    w.extensions = p
        .config
        .extensions
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    w.mimes = p
        .config
        .mime_types
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    w.depth = p.config.limits.depth;
    w.urls = p.config.limits.urls;
    w.budget_mib = p.config.limits.body_bytes >> 20;
    w.private_seed = p.config.allow_private_seed;
    w.file_includes = p.rules.include.join(" ");
    w.file_excludes = p.rules.exclude.join(" ");
    w.minimum_mib = p
        .rules
        .minimum_bytes
        .map(|n| (n >> 20).to_string())
        .unwrap_or_default();
    w.maximum_mib = p
        .rules
        .maximum_bytes
        .map(|n| (n >> 20).to_string())
        .unwrap_or_default();
    w.auto_download = p.auto_download;
    w.layout = p.layout;
    w.collision = p.collision;
    w.scheduled_action = p.scheduled_action;
    if let Some(s) = &p.schedule {
        w.scheduled = true;
        w.hour = s.hour;
        w.minute = s.minute;
        w.days = s.days;
        w.once = s
            .once
            .map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}"))
            .unwrap_or_default();
    }
    w
}
fn explore_label(status: ExploreStatus) -> &'static str {
    match status {
        ExploreStatus::Ready => t("Ready"),
        ExploreStatus::Exploring => t("Exploring"),
        ExploreStatus::Stopped => t("Stopped"),
        ExploreStatus::Completed => t("Completed"),
        ExploreStatus::Limited => t("Limited"),
        ExploreStatus::Failed => t("Failed"),
    }
}
fn transfer_label(status: sdl_grabber::catalog::TransferStatus) -> &'static str {
    use sdl_grabber::catalog::TransferStatus::*;
    match status {
        Available => t("Available"),
        Queued => t("Queued"),
        Downloading => t("Downloading"),
        Completed => t("Completed"),
        Failed => t("Failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn site_grabber_memory_credentials_keep_their_origin_and_header_kind() {
        let mut workspace = FilesWorkspace::new(&tokio::runtime::Handle::current());
        let id = ProjectId::parse("session-boundary").unwrap();
        let old = CredentialRef {
            identifier: "memory-only".into(),
            origin: "https://old.example".into(),
        };
        let new = CredentialRef {
            identifier: "memory-only".into(),
            origin: "https://new.example".into(),
        };
        let cookie = CredentialRef {
            identifier: "memory-cookie".into(),
            origin: old.origin.clone(),
        };
        workspace
            .memory
            .insert(session_key(&id, &old), "old-authorization".into());
        workspace
            .memory
            .insert(session_key(&id, &new), "new-authorization".into());
        workspace
            .memory
            .insert(session_key(&id, &cookie), "old-cookie".into());
        assert_eq!(
            workspace.session_for(&id, &old).unwrap(),
            "old-authorization"
        );
        assert_eq!(
            workspace.session_for(&id, &new).unwrap(),
            "new-authorization"
        );
        assert_eq!(workspace.session_for(&id, &cookie).unwrap(), "old-cookie");
    }
    #[test]
    fn site_grabber_protected_session_edits_do_not_overwrite_queued_references() {
        let id = ProjectId::parse("protected-reference").unwrap();
        let w = default_wizard();
        let mut p = FileProject::new(ProjectConfig::new(
            "Files",
            "https://old.example/".parse().unwrap(),
            "output".into(),
        ));
        let old = protected_reference(&id, &p);
        p.config.seed = "https://new.example/".parse().unwrap();
        let other_origin = protected_reference(&id, &p);
        assert_ne!(old.identifier, other_origin.identifier);
        p.rescan();
        let other_revision = protected_reference(&id, &p);
        assert_ne!(other_revision.identifier, other_origin.identifier);
        assert!(!w.auto_download);
    }
    #[test]
    fn site_grabber_browser_session_cannot_follow_an_edited_seed_origin() {
        let mut project = FileProject::new(ProjectConfig::new(
            "Files",
            "https://source.example/".parse().unwrap(),
            "output".into(),
        ));
        assert!(browser_reference("https://source.example", &project).is_ok());
        project.config.seed = "https://other.example/".parse().unwrap();
        assert!(browser_reference("https://source.example", &project).is_err());
    }
    #[test]
    fn site_grabber_file_wizard_defaults_to_explore_only_with_no_transfer_ceiling() {
        let w = default_wizard();
        assert_eq!(w.mode, ProjectMode::Files);
        assert!(!w.auto_download);
        assert_eq!(w.budget_mib, 64);
        assert!(file_rules(&w).unwrap().maximum_bytes.is_none());
    }
    #[test]
    fn site_grabber_file_size_filters_reject_inverted_ranges() {
        let mut w = default_wizard();
        w.minimum_mib = "2".into();
        w.maximum_mib = "1".into();
        assert!(file_rules(&w).is_err());
    }
}
