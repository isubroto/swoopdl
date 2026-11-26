//! Native, theme-driven project workspace and four-step capture wizard.
use super::*;
use egui::{Align, Button, Layout, Rect, RichText, Ui, UiBuilder, vec2};

const WIDTH: f32 = 820.0;
const HEIGHT: f32 = 620.0;

fn region<R>(ui: &mut Ui, rect: Rect, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.set_width(rect.width());
            ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
            body(ui)
        },
    )
    .inner
}
pub(super) fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(crate::theme::colors().muted));
}
pub(super) fn field(ui: &mut Ui, label: &str, value: &mut String, password: bool) {
    let label = ui.label(label);
    ui.add_sized(
        vec2(ui.available_width(), 30.0),
        egui::TextEdit::singleline(value).password(password),
    )
    .labelled_by(label.id);
}
fn primary(ui: &mut Ui, label: &str, enabled: bool) -> egui::Response {
    let c = crate::theme::colors();
    ui.add_enabled(
        enabled,
        Button::new(RichText::new(label).color(c.button_accent_text))
            .fill(c.button_accent)
            .min_size(vec2(112.0, 32.0)),
    )
}
fn secondary(ui: &mut Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(enabled, Button::new(label).min_size(vec2(88.0, 32.0)))
}
fn status(value: ProjectStatus) -> &'static str {
    match value {
        ProjectStatus::Draft => t("Draft"),
        ProjectStatus::Running => t("Running"),
        ProjectStatus::Pausing => t("Pausing"),
        ProjectStatus::Paused => t("Paused"),
        ProjectStatus::Completed => t("Completed"),
        ProjectStatus::Limited => t("Limited"),
        ProjectStatus::Failed => t("Failed"),
    }
}

impl SiteGrabber {
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome<SiteAction> {
        if !self.open {
            return Outcome::Closed;
        }
        if self.show_files {
            return self.files.show(ctx, &mut self.open, &mut self.show_files);
        }
        let mut action = None;
        let outcome = crate::dialogs::window(
            ctx,
            "site-grabber",
            "Site Grabber",
            [WIDTH, HEIGHT],
            true,
            crate::theme::colors().window,
            |ui, origin| {
                let rect = Rect::from_min_size(origin, vec2(WIDTH, HEIGHT));
                match region(ui, rect, |ui| self.body(ui, rect)) {
                    Outcome::Done(value) => {
                        action = Some(value);
                        Outcome::<()>::Open
                    }
                    Outcome::Open => Outcome::Open,
                    Outcome::Closed => Outcome::Closed,
                }
            },
        );
        if matches!(outcome, Outcome::Closed) {
            self.open = false;
            self.wizard.authorization.clear();
        }
        match action {
            Some(action) => Outcome::Done(action),
            None if matches!(outcome, Outcome::Closed) => Outcome::Closed,
            None => Outcome::Open,
        }
    }

    fn body(&mut self, ui: &mut Ui, bounds: Rect) -> Outcome<SiteAction> {
        let c = crate::theme::colors();
        let busy = self.saving || self.folder_pending || self.validating_destination;
        let at = bounds.min;
        let mut action = None;
        ui.visuals_mut().override_text_color = Some(c.text);
        region(
            ui,
            Rect::from_min_size(at + vec2(24.0, 16.0), vec2(772.0, 56.0)),
            |ui| {
                ui.horizontal(|ui| {
                    if ui.button(t("Files")).clicked() {
                        self.show_files = true;
                    }
                    ui.label(
                        RichText::new(if self.creating {
                            t("New capture project")
                        } else {
                            t("Site Grabber")
                        })
                        .size(23.0)
                        .strong(),
                    );
                    if self.creating {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            note(
                                ui,
                                &crate::i18n::fmt(
                                    t("Step {step} of 4"),
                                    &[("step", &(self.wizard.step.index() + 1).to_string())],
                                ),
                            );
                        });
                    }
                });
                note(
                    ui,
                    if self.creating {
                        t("Set up a website capture in four simple steps.")
                    } else {
                        t("Capture files from websites. Keep an offline copy. Resume anytime.")
                    },
                );
            },
        );
        let rail = Rect::from_min_size(at + vec2(0.0, 86.0), vec2(208.0, 468.0));
        ui.painter().rect_filled(rail, 0.0, c.egui_panel);
        ui.painter().line_segment(
            [at + vec2(0.0, 85.0), at + vec2(WIDTH, 85.0)],
            egui::Stroke::new(1.0, c.egui_window_border),
        );
        region(ui, rail.shrink2(vec2(16.0, 20.0)), |ui| {
            if self.creating {
                for step in WizardStep::ALL {
                    let current = self.wizard.step == step;
                    let text = format!("{}.  {}", step.index() + 1, step.title());
                    let button = Button::new(RichText::new(text).color(if current {
                        c.button_accent_text
                    } else {
                        c.text
                    }))
                    .fill(if current {
                        c.button_accent
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .stroke(egui::Stroke::NONE)
                    .min_size(vec2(176.0, 48.0))
                    .wrap();
                    if ui
                        .add_enabled(!busy && step.index() <= self.wizard.step.index(), button)
                        .clicked()
                    {
                        self.wizard.step = step;
                        self.error = None;
                    }
                }
                ui.add_space(24.0);
                note(ui, t("Your settings stay with you when you go back."));
            } else {
                ui.label(RichText::new(t("Saved projects")).strong());
                egui::ScrollArea::vertical()
                    .max_height(350.0)
                    .show(ui, |ui| {
                        for project in &self.projects {
                            let selected = self.selected.as_ref() == Some(&project.id);
                            let text = format!(
                                "{}\n{}",
                                project.state.config.name,
                                status(project.state.status)
                            );
                            if ui
                                .add(
                                    Button::new(text)
                                        .selected(selected)
                                        .min_size(vec2(176.0, 52.0))
                                        .wrap(),
                                )
                                .clicked()
                            {
                                self.selected = Some(project.id.clone());
                            }
                        }
                        if !self.loaded {
                            ui.spinner();
                        } else if self.projects.is_empty() {
                            note(ui, t("No projects yet"));
                        }
                    });
            }
        });
        let main = Rect::from_min_size(at + vec2(232.0, 106.0), vec2(564.0, 428.0));
        region(ui, main, |ui| {
            egui::ScrollArea::vertical()
                .id_salt((
                    "site-grabber-content",
                    self.creating,
                    self.wizard.step.index(),
                ))
                .max_height(main.height())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.creating {
                        ui.heading(self.wizard.step.title());
                        note(ui, self.wizard.step.description());
                        ui.add_space(6.0);
                        if ui
                            .add_enabled_ui(!self.saving && !self.validating_destination, |ui| {
                                self.wizard.fields(ui, self.folder_pending)
                            })
                            .inner
                        {
                            action = Some(SiteAction::BrowseDestination);
                        }
                    } else {
                        self.project_details(ui, &mut action);
                    }
                    if let Some(error) = &self.error {
                        ui.colored_label(c.error, error);
                    }
                });
        });
        ui.painter().line_segment(
            [at + vec2(0.0, 554.0), at + vec2(WIDTH, 554.0)],
            egui::Stroke::new(1.0, c.egui_window_border),
        );
        region(
            ui,
            Rect::from_min_size(at + vec2(24.0, 574.0), vec2(772.0, 32.0)),
            |ui| {
                ui.horizontal(|ui| {
                    if self.creating {
                        if secondary(ui, t("Cancel"), !busy).clicked() {
                            self.creating = false;
                            self.wizard.authorization.clear();
                            self.error = None;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if self.wizard.step == WizardStep::Filters {
                                if primary(
                                    ui,
                                    if self.saving {
                                        t("Saving project…")
                                    } else {
                                        t("Save project")
                                    },
                                    !busy,
                                )
                                .clicked()
                                {
                                    match self.wizard.config() {
                                        Ok(config) => {
                                            self.error = None;
                                            action = Some(SiteAction::Create(Box::new(config)));
                                        }
                                        Err(error) => self.error = Some(error),
                                    }
                                }
                            } else if primary(
                                ui,
                                if self.validating_destination {
                                    t("Checking folder…")
                                } else {
                                    t("Next")
                                },
                                !busy,
                            )
                            .clicked()
                            {
                                if self.wizard.step == WizardStep::Destination {
                                    match self
                                        .wizard
                                        .validate_step()
                                        .and_then(|()| self.wizard.destination_path())
                                    {
                                        Ok(path) => {
                                            self.error = None;
                                            action = Some(SiteAction::ValidateDestination(path));
                                        }
                                        Err(error) => self.error = Some(error),
                                    }
                                } else {
                                    self.error = self.wizard.advance().err();
                                }
                            }
                            if secondary(
                                ui,
                                t("Back"),
                                !busy && self.wizard.step != WizardStep::Project,
                            )
                            .clicked()
                            {
                                self.wizard.back();
                                self.error = None;
                            }
                        });
                    } else {
                        if primary(ui, t("New project"), !busy).clicked() {
                            self.wizard = Wizard::default();
                            self.creating = true;
                            self.error = None;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if secondary(ui, t("Close"), true).clicked() {
                                self.open = false;
                            }
                        });
                    }
                });
            },
        );
        if let Some(action) = action {
            Outcome::Done(action)
        } else if self.open {
            Outcome::Open
        } else {
            Outcome::Closed
        }
    }

    fn project_details(&mut self, ui: &mut Ui, action: &mut Option<SiteAction>) {
        let Some(p) = self
            .projects
            .iter()
            .find(|p| Some(&p.id) == self.selected.as_ref())
        else {
            ui.add_space(40.0);
            ui.heading(t("Your next website, saved locally"));
            note(
                ui,
                t(
                    "Start with a page address. Choose images, documents, media, or a complete offline website.",
                ),
            );
            ui.add_space(12.0);
            note(
                ui,
                t("Create a project to get started, or select a saved project on the left."),
            );
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.heading(&p.state.config.name);
            ui.label(status(p.state.status));
        });
        ui.label(sdl_grabber::report::sanitized_url(&p.state.config.seed));
        note(ui, &p.state.config.destination.to_string_lossy());
        ui.add_space(4.0);
        let progress = self.active.as_ref().filter(|a| a.id == p.id);
        let idle = self.active.is_none() && !self.folder_pending && !self.validating_destination;
        egui::Grid::new("capture-stats")
            .num_columns(3)
            .spacing(vec2(28.0, 6.0))
            .show(ui, |ui| {
                for label in [t("Saved files"), t("Pending URLs"), t("Downloaded")] {
                    note(ui, label);
                }
                ui.end_row();
                ui.label(
                    RichText::new(
                        progress
                            .map_or(p.state.resources.len(), |a| a.saved)
                            .to_string(),
                    )
                    .size(20.0),
                );
                ui.label(
                    RichText::new(
                        progress
                            .map_or(p.state.pending.len(), |a| a.pending)
                            .to_string(),
                    )
                    .size(20.0),
                );
                ui.label(RichText::new(crate::model::size(p.state.consumed_bytes)).size(20.0));
                ui.end_row();
            });
        let budget = p.state.config.limits.body_bytes.max(1);
        ui.add(
            egui::ProgressBar::new((p.state.consumed_bytes as f64 / budget as f64).min(1.0) as f32)
                .text(t("Run budget used"))
                .desired_width(ui.available_width()),
        );
        if p.pause_pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(t("Pausing: waiting for durable checkpoint"));
            });
        }
        if let Some(error) = &p.error {
            ui.colored_label(crate::theme::colors().error, error);
        }
        if let Some(reason) = p.state.reports.last() {
            note(ui, &reason.reason);
        }
        ui.horizontal_wrapped(|ui| {
            let label = match p.state.status {
                ProjectStatus::Draft => t("Start capture"),
                ProjectStatus::Completed | ProjectStatus::Limited => t("Run again"),
                ProjectStatus::Failed => t("Retry capture"),
                _ => t("Start / Resume"),
            };
            if primary(ui, label, idle).clicked() {
                *action = Some(SiteAction::Start(p.id.clone()));
            }
            if secondary(ui, t("Pause"), progress.is_some() && !p.pause_pending).clicked() {
                *action = Some(SiteAction::Pause(p.id.clone()));
            }
            if secondary(ui, t("Open offline copy"), p.state.generation > 0 && idle).clicked() {
                *action = Some(SiteAction::OpenOutput(p.id.clone()));
            }
            if can_change_destination(&p.state)
                && secondary(
                    ui,
                    if self.folder_pending {
                        t("Choosing folder…")
                    } else {
                        t("Change destination")
                    },
                    idle,
                )
                .clicked()
            {
                *action = Some(SiteAction::ChangeDestination(p.id.clone()));
            }
        });
        ui.collapsing(t("Project options"), |ui| {
            if p.state.credential.is_some() && idle {
                field(
                    ui,
                    t("Replacement session (optional, memory only)"),
                    &mut self.session,
                    true,
                );
            }
            ui.horizontal_wrapped(|ui| {
                if secondary(
                    ui,
                    t("Synchronize"),
                    idle && (p.state.generation > 0 || p.state.consumed_bytes >= budget),
                )
                .clicked()
                {
                    *action = Some(SiteAction::Synchronize(p.id.clone()));
                }
                if secondary(ui, t("Export report"), idle).clicked() {
                    *action = Some(SiteAction::ExportReport(p.id.clone()));
                }
            });
            if ui
                .add_enabled(
                    idle,
                    Button::new(t("Delete metadata (keep captured files)")),
                )
                .clicked()
            {
                *action = Some(SiteAction::DeleteMetadata(p.id.clone()));
            }
        });
    }

    /// Explicit /show previews use draft data only; they never start a capture.
    pub(crate) fn preview(&mut self, stage: &str) {
        if stage.starts_with("files-") {
            self.show_files = true;
            self.files.preview(stage);
            return;
        }
        let step = match stage {
            "project" => WizardStep::Project,
            "destination" => WizardStep::Destination,
            "explore" => WizardStep::Explore,
            "filters" => WizardStep::Filters,
            _ => return,
        };
        self.show_files = false;
        self.wizard = Wizard {
            name: t("Website images").into(),
            seed: "https://example.com/gallery/".into(),
            destination: "C:\\Downloads".into(),
            step,
            ..Default::default()
        };
        self.wizard.apply_template(ProjectTemplate::Images);
        self.creating = true;
    }
}

impl Wizard {
    pub(super) fn fields(&mut self, ui: &mut Ui, folder_pending: bool) -> bool {
        match self.step {
            WizardStep::Project => {
                field(ui, t("Project name"), &mut self.name, false);
                field(ui, t("Start page address"), &mut self.seed, false);
                let label = ui.label(t("Project template"));
                let mut template = self.template;
                egui::ComboBox::from_id_salt("capture-template")
                    .selected_text(template.title())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for choice in ProjectTemplate::ALL {
                            if self.mode == sdl_grabber::ProjectMode::Files
                                && choice == ProjectTemplate::OfflineSite
                            {
                                continue;
                            }
                            ui.selectable_value(&mut template, choice, choice.title());
                        }
                    })
                    .response
                    .labelled_by(label.id);
                if template != self.template {
                    self.apply_template(template);
                }
                note(
                    ui,
                    t("Templates set file filters for you. You can change them in the last step."),
                );
                ui.collapsing(t("Authorization (optional)"), |ui| {
                    ui.checkbox(
                        &mut self.authenticated,
                        t("Use an origin-scoped authorization header"),
                    );
                    if self.authenticated {
                        field(ui, t("Authorization header"), &mut self.authorization, true);
                        ui.checkbox(
                            &mut self.protect,
                            t("Save in system credential vault (otherwise memory only)"),
                        );
                    }
                });
            }
            WizardStep::Destination => {
                let label = ui.label(t("Base folder"));
                let browse = ui
                    .horizontal(|ui| {
                        let width = (ui.available_width() - 100.0).max(100.0);
                        ui.add_sized(
                            vec2(width, 30.0),
                            egui::TextEdit::singleline(&mut self.destination),
                        )
                        .labelled_by(label.id);
                        secondary(
                            ui,
                            if folder_pending {
                                t("Choosing folder…")
                            } else {
                                t("Browse…")
                            },
                            !folder_pending,
                        )
                        .clicked()
                    })
                    .inner;
                if browse {
                    return true;
                }
                ui.checkbox(
                    &mut self.project_subfolder,
                    t("Create a project subfolder (recommended)"),
                );
                if self.project_subfolder {
                    let label = ui.label(t("Project subfolder"));
                    ui.add_sized(
                        vec2(ui.available_width(), 30.0),
                        egui::TextEdit::singleline(&mut self.project_folder)
                            .hint_text(wizard::project_folder_name(&self.name)),
                    )
                    .labelled_by(label.id);
                }
                ui.label(RichText::new(t("Files will be saved to")).strong());
                if let Ok(destination) = self.destination_path() {
                    note(ui, &destination.to_string_lossy());
                }
                note(
                    ui,
                    if self.mode == sdl_grabber::ProjectMode::Files {
                        t("Existing folders are supported. Name collisions receive numbered names.")
                    } else {
                        t(
                            "The final project folder must be empty. Other files in the base folder are left untouched.",
                        )
                    },
                );
                if self.mode == sdl_grabber::ProjectMode::Files {
                    ui.label(t("When a filename already exists"));
                    egui::ComboBox::from_id_salt("file-collision")
                        .selected_text(
                            if self.collision == sdl_grabber::catalog::CollisionPolicy::Number {
                                t("Number duplicate names")
                            } else {
                                t("Skip existing files")
                            },
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.collision,
                                sdl_grabber::catalog::CollisionPolicy::Number,
                                t("Number duplicate names"),
                            );
                            ui.selectable_value(
                                &mut self.collision,
                                sdl_grabber::catalog::CollisionPolicy::Skip,
                                t("Skip existing files"),
                            );
                        });
                    ui.label(t("Save layout"));
                    egui::ComboBox::from_id_salt("file-layout")
                        .selected_text(layout_label(self.layout))
                        .show_ui(ui, |ui| {
                            for layout in [
                                sdl_grabber::catalog::SaveLayout::Flat,
                                sdl_grabber::catalog::SaveLayout::OriginalSubfolders,
                                sdl_grabber::catalog::SaveLayout::Category,
                            ] {
                                ui.selectable_value(&mut self.layout, layout, layout_label(layout));
                            }
                        });
                }
                ui.collapsing(t("Synchronization schedule (optional)"), |ui| {
                    ui.checkbox(
                        &mut self.scheduled,
                        t("Schedule synchronization while the app is open"),
                    );
                    if self.scheduled {
                        if self.mode == sdl_grabber::ProjectMode::Files {
                            ui.label(t("Scheduled action"));
                            egui::ComboBox::from_id_salt("file-scheduled-action")
                                .selected_text(action_label(self.scheduled_action))
                                .show_ui(ui, |ui| {
                                    for action in [
                                        sdl_grabber::catalog::ScheduledAction::ExploreOnly,
                                        sdl_grabber::catalog::ScheduledAction::ExploreAndDownload,
                                        sdl_grabber::catalog::ScheduledAction::DownloadChecked,
                                        sdl_grabber::catalog::ScheduledAction::Synchronize,
                                    ] {
                                        ui.selectable_value(
                                            &mut self.scheduled_action,
                                            action,
                                            action_label(action),
                                        );
                                    }
                                });
                        }
                        ui.horizontal(|ui| {
                            ui.label(t("Local time"));
                            ui.add(egui::DragValue::new(&mut self.hour).range(0..=23));
                            ui.label(":");
                            ui.add(egui::DragValue::new(&mut self.minute).range(0..=59));
                        });
                        ui.horizontal_wrapped(|ui| {
                            for (i, day) in [
                                t("Sun"),
                                t("Mon"),
                                t("Tue"),
                                t("Wed"),
                                t("Thu"),
                                t("Fri"),
                                t("Sat"),
                            ]
                            .iter()
                            .enumerate()
                            {
                                ui.checkbox(&mut self.days[i], *day);
                            }
                        });
                        field(
                            ui,
                            t("One-time date (YYYY-MM-DD, blank repeats)"),
                            &mut self.once,
                            false,
                        );
                    }
                });
            }
            WizardStep::Explore => {
                ui.horizontal(|ui| {
                    let label = ui.label(t("Link levels to explore"));
                    ui.add(egui::DragValue::new(&mut self.depth).range(0..=100))
                        .labelled_by(label.id);
                });
                note(
                    ui,
                    t(
                        "Level 0 captures the start page and its assets. Each next level follows another page link.",
                    ),
                );
                ui.label(RichText::new(t("Page boundaries")).strong());
                ui.radio_value(
                    &mut self.restrict_directory,
                    true,
                    t("Stay in the starting page's folder"),
                );
                ui.radio_value(
                    &mut self.restrict_directory,
                    false,
                    t("Explore the whole website on this origin"),
                );
                note(
                    ui,
                    t("Other page origins are blocked unless you explicitly add them below."),
                );
                ui.collapsing(t("Advanced exploration rules"), |ui| {
                    field(
                        ui,
                        t("Page directory scope (blank uses selection above)"),
                        &mut self.path_prefix,
                        false,
                    );
                    field(
                        ui,
                        t("Additional page origins (space separated)"),
                        &mut self.page_origins,
                        false,
                    );
                    field(ui, t("Include URL prefixes"), &mut self.includes, false);
                    field(ui, t("Exclude URL prefixes"), &mut self.excludes, false);
                    ui.checkbox(
                        &mut self.private_seed,
                        t("Allow private addresses for the exact seed origin"),
                    );
                });
            }
            WizardStep::Filters => {
                field(
                    ui,
                    t("File extensions (space separated, blank allows all)"),
                    &mut self.extensions,
                    false,
                );
                note(ui, self.extension_hint());
                egui::Grid::new("capture-limits")
                    .spacing(vec2(20.0, 12.0))
                    .show(ui, |ui| {
                        if self.mode == sdl_grabber::ProjectMode::OfflineSite {
                            let label = ui.label(t("Maximum file size (MiB)"));
                            ui.add(egui::DragValue::new(&mut self.resource_mib).range(1..=1024))
                                .labelled_by(label.id);
                            ui.end_row();
                        }
                        let label = ui.label(t("Maximum unique URLs"));
                        ui.add(egui::DragValue::new(&mut self.urls).range(1..=100_000))
                            .labelled_by(label.id);
                        ui.end_row();
                        let label = ui.label(t("Run body budget (MiB)"));
                        ui.add(egui::DragValue::new(&mut self.budget_mib).range(1..=1_048_576))
                            .labelled_by(label.id);
                        ui.end_row();
                    });
                ui.collapsing(t("Advanced file filters"), |ui| {
                    field(
                        ui,
                        t("Asset MIME types (blank allows all)"),
                        &mut self.mimes,
                        false,
                    );
                    field(
                        ui,
                        t("Additional asset origins (space separated)"),
                        &mut self.asset_origins,
                        false,
                    );
                    note(
                        ui,
                        t("Asset origins allow files, not navigation to other websites."),
                    );
                });
                if self.mode == sdl_grabber::ProjectMode::Files {
                    field(
                        ui,
                        t("Include file patterns (space separated)"),
                        &mut self.file_includes,
                        false,
                    );
                    field(
                        ui,
                        t("Exclude file patterns (space separated)"),
                        &mut self.file_excludes,
                        false,
                    );
                    field(
                        ui,
                        t("Minimum size (MiB, blank allows any)"),
                        &mut self.minimum_mib,
                        false,
                    );
                    field(
                        ui,
                        t("Maximum size (MiB, blank allows any)"),
                        &mut self.maximum_mib,
                        false,
                    );
                    ui.checkbox(
                        &mut self.auto_download,
                        t("Automatically download newly matched files"),
                    );
                    note(
                        ui,
                        t(
                            "Exploration reads pages and bounded metadata. Downloads require checked files.",
                        ),
                    );
                }
                ui.separator();
                ui.label(RichText::new(t("Ready to save")).strong());
                if let Ok(destination) = self.destination_path() {
                    note(ui, &destination.to_string_lossy());
                }
                note(
                    ui,
                    if self.mode == sdl_grabber::ProjectMode::Files {
                        t(
                            "Save the project, then choose Start Exploring. Robots rules apply. Static HTML/CSS only; scripts and forms stay disabled.",
                        )
                    } else {
                        t(
                            "Save the project, then choose Start capture. Robots rules apply. Static HTML/CSS only; scripts and forms stay disabled.",
                        )
                    },
                );
            }
        }
        false
    }
}

pub(super) fn layout_label(layout: sdl_grabber::catalog::SaveLayout) -> &'static str {
    match layout {
        sdl_grabber::catalog::SaveLayout::Flat => t("One folder"),
        sdl_grabber::catalog::SaveLayout::OriginalSubfolders => t("Original subfolders"),
        sdl_grabber::catalog::SaveLayout::Category => t("Category folders"),
    }
}
pub(super) fn action_label(action: sdl_grabber::catalog::ScheduledAction) -> &'static str {
    match action {
        sdl_grabber::catalog::ScheduledAction::ExploreOnly => t("Explore only"),
        sdl_grabber::catalog::ScheduledAction::ExploreAndDownload => t("Explore and download"),
        sdl_grabber::catalog::ScheduledAction::DownloadChecked => t("Download checked files"),
        sdl_grabber::catalog::ScheduledAction::Synchronize => t("Synchronize"),
    }
}
