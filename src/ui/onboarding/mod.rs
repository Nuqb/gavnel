//! First-run setup edits a private draft. Only the host commits Finish or Skip.
mod audio_page;
mod capture;
mod integration;
mod layout;
pub(super) use integration::open_preview;
mod preferences;
pub(crate) mod preview;
mod recording;
#[cfg(test)]
mod tests;

use super::*;
use egui::{Align, Layout, Sense, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Welcome,
    Capture,
    Quality,
    Hotkeys,
    Audio,
    Preferences,
    Ready,
}
impl Step {
    const ALL: [Self; 7] = [
        Self::Welcome,
        Self::Capture,
        Self::Quality,
        Self::Hotkeys,
        Self::Audio,
        Self::Preferences,
        Self::Ready,
    ];
    fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap()
    }
    fn label(self) -> &'static str {
        tr(match self {
            Self::Welcome => "Welcome",
            Self::Capture => "Capture source",
            Self::Quality => "Recording quality",
            Self::Hotkeys => "Your hotkeys",
            Self::Audio => "Audio",
            Self::Preferences => "Preferences",
            Self::Ready => "Ready to go",
        })
    }
}

pub(super) struct Devices<'a> {
    monitors: &'a [MonitorInfo],
    windows: &'a [WindowInfo],
    inputs: &'a [AudioDevice],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Skip,
    Finish,
    BrowseFolder,
    Refresh,
}

pub(super) struct Wizard {
    draft: Settings,
    step: Step,
    folder: String,
    listening: Option<u64>,
    error: String,
    start_capture: bool,
    preview: bool,
    entered_at: Option<f64>,
    mic_test: Option<audio::MicTest>,
}

impl Wizard {
    pub(super) fn new(draft: Settings, armed: bool) -> Self {
        Self {
            folder: draft.clips_dir.to_string_lossy().into_owned(),
            draft,
            step: Step::Welcome,
            listening: None,
            error: String::new(),
            start_capture: armed,
            preview: false,
            entered_at: None,
            mic_test: None,
        }
    }

    fn candidate(&self) -> Result<Settings> {
        let mut settings = self.draft.clone();
        settings.clips_dir = PathBuf::from(self.folder.trim());
        settings.onboarding_completed = true;
        settings.capture_enabled =
            Some(self.start_capture && settings.recording_mode != RecordingMode::Off);
        settings.validate()?;
        self.validate_source()?;
        super::shortcuts::parse_bindings(&settings)?;
        Ok(settings)
    }

    fn skipped_settings(&self, current: &Settings) -> Settings {
        let mut saved = current.clone();
        saved.onboarding_completed = true;
        saved.language = self.draft.language;
        saved
    }

    fn validate_source(&self) -> Result<()> {
        anyhow::ensure!(
            self.draft.mode != CaptureMode::Window || !self.draft.window_title.is_empty(),
            "Choose a window, or use automatic game detection."
        );
        anyhow::ensure!(
            self.draft.mode != CaptureMode::Monitor || self.draft.monitor_index > 0,
            "Choose a display."
        );
        Ok(())
    }

    fn advance(&mut self) {
        let result = match self.step {
            Step::Capture => self.validate_source(),
            Step::Hotkeys => super::shortcuts::parse_bindings(&self.draft).map(|_| ()),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.error = error.to_string();
            return;
        }
        self.go_to(Step::ALL[(self.step.index() + 1).min(6)]);
    }

    fn go_to(&mut self, step: Step) {
        self.step = step;
        self.entered_at = None;
        self.error.clear();
        self.listening = None;
        self.mic_test = None;
    }

    fn content(&mut self, ui: &mut egui::Ui, devices: &Devices<'_>, action: &mut Option<Action>) {
        match self.step {
            Step::Welcome => {}
            Step::Capture => self.capture_page(ui, devices, action),
            Step::Quality => self.quality_page(ui),
            Step::Hotkeys => self.hotkey_page(ui),
            Step::Audio => self.audio_page(ui, devices),
            Step::Preferences => self.preferences_page(ui, action),
            Step::Ready => self.review(ui),
        }
    }
}
