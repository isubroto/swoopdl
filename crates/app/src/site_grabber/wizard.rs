//! Four-stage setup state; presentation lives in view.rs.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum WizardStep {
    #[default]
    Project,
    Destination,
    Explore,
    Filters,
}
impl WizardStep {
    pub const ALL: [Self; 4] = [
        Self::Project,
        Self::Destination,
        Self::Explore,
        Self::Filters,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Project => t("Project & start page"),
            Self::Destination => t("Save location"),
            Self::Explore => t("Explore rules"),
            Self::Filters => t("File filters"),
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Project => t("Name your project and choose what to capture."),
            Self::Destination => t("Choose a folder for this capture and its offline copy."),
            Self::Explore => t("Choose how far to follow links and where to stay."),
            Self::Filters => t("Choose file types and review your capture limits."),
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ProjectTemplate {
    #[default]
    OfflineSite,
    Images,
    Audio,
    Video,
    Documents,
    Custom,
}
impl ProjectTemplate {
    pub const ALL: [Self; 6] = [
        Self::OfflineSite,
        Self::Images,
        Self::Audio,
        Self::Video,
        Self::Documents,
        Self::Custom,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::OfflineSite => t("Website for offline browsing"),
            Self::Images => t("Images from a website"),
            Self::Audio => t("Audio files"),
            Self::Video => t("Video files"),
            Self::Documents => t("Documents"),
            Self::Custom => t("Custom settings"),
        }
    }
}
impl Wizard {
    pub fn destination_path(&self) -> Result<PathBuf, String> {
        if self.destination.trim().is_empty() {
            return Err(t("Choose a destination folder").into());
        }
        let base = PathBuf::from(self.destination.trim());
        if !self.project_subfolder {
            return Ok(base);
        }
        let folder = if self.project_folder.is_empty() {
            project_folder_name(&self.name)
        } else {
            self.project_folder.clone()
        };
        validate_project_folder(&folder)?;
        Ok(base.join(folder))
    }

    pub fn accept_destination_check(
        &mut self,
        checked: &std::path::Path,
        result: Result<(), String>,
    ) -> Result<(), String> {
        if self.step != WizardStep::Destination || self.destination_path()?.as_path() != checked {
            return Err(t("Destination changed. Check the folder again.").into());
        }
        result?;
        self.advance()
    }

    pub fn extension_hint(&self) -> &'static str {
        match self.template {
            ProjectTemplate::Video => t(
                "Video files can exceed these limits. Review the per-file cap and total budget before starting.",
            ),
            ProjectTemplate::Audio => {
                t("Example: mp3 m4a aac flac ogg opus wav. HTML pages are kept for navigation.")
            }
            ProjectTemplate::Documents => {
                t("Example: pdf docx xlsx pptx epub. HTML pages are kept for navigation.")
            }
            ProjectTemplate::Images => {
                t("Example: jpg jpeg png gif webp. HTML pages are kept for navigation.")
            }
            _ => t(
                "Leave extensions blank to allow all file types. HTML pages are kept for navigation.",
            ),
        }
    }
    pub fn apply_template(&mut self, template: ProjectTemplate) {
        self.template = template;
        if template == ProjectTemplate::Custom {
            return;
        }
        self.extensions = match template {
            ProjectTemplate::Images => "jpg jpeg png gif webp avif svg ico bmp tif tiff",
            ProjectTemplate::Audio => "mp3 m4a aac flac ogg opus wav",
            ProjectTemplate::Video => "mp4 webm mkv mov m4v avi",
            ProjectTemplate::Documents => "pdf doc docx xls xlsx ppt pptx txt odt ods epub",
            _ => "",
        }
        .into();
        self.mimes.clear();
    }
    pub fn validate_step(&self) -> Result<(), String> {
        match self.step {
            WizardStep::Project => {
                let seed = Url::parse(self.seed.trim())
                    .map_err(|_| t("Enter a valid HTTP(S) start URL").to_owned())?;
                ProjectConfig::new(self.name.trim(), seed, PathBuf::from("capture")).validate()?;
                if self.authenticated {
                    if self.authorization.trim().is_empty() {
                        return Err(t("Enter session credentials").into());
                    }
                    self.authorization
                        .parse::<reqwest::header::HeaderValue>()
                        .map_err(|_| t("Invalid authorization header").to_owned())?;
                }
                Ok(())
            }
            WizardStep::Destination => {
                self.destination_path()?;
                self.schedule().map(|_| ())
            }
            WizardStep::Explore | WizardStep::Filters => self.config().map(|_| ()),
        }
    }
    pub fn advance(&mut self) -> Result<(), String> {
        self.validate_step()?;
        self.step = WizardStep::ALL[(self.step.index() + 1).min(3)];
        Ok(())
    }
    pub fn back(&mut self) {
        self.step = WizardStep::ALL[self.step.index().saturating_sub(1)];
    }
}

fn validate_project_folder(folder: &str) -> Result<(), String> {
    let stem = folder
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    // Win32 also treats ISO-8859-1 superscript 1/2/3 as COM/LPT device digits.
    let port = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || port.is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    });
    if folder.is_empty()
        || folder.chars().count() > 80
        || matches!(folder, "." | "..")
        || folder.ends_with([' ', '.'])
        || reserved
        || folder
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(t(
            "Use one valid subfolder name without slashes, reserved names, or trailing dots.",
        )
        .into());
    }
    Ok(())
}

pub(super) fn project_folder_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .take(64)
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches([' ', '.']);
    if cleaned.is_empty() {
        "capture".into()
    } else if validate_project_folder(cleaned).is_err() {
        format!("capture-{cleaned}")
    } else {
        cleaned.into()
    }
}
