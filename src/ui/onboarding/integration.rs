//! Host-side effects live here, outside the wizard's draft and presentation.
use super::*;

impl Gavnel {
    pub(in crate::ui) fn begin_onboarding(&mut self) {
        self.onboarding = Some(Wizard::new(
            self.settings.clone(),
            self.engine.snapshot().armed,
        ));
        self.quick_open = false;
        self.clips_open = false;
        self.long_recording_menu_open = false;
        self.source_picker = None;
        self.mic_test = None;
        self.capturing_hotkey.store(true, Ordering::Relaxed);
        self.pressed_hotkeys.lock().unwrap().clear();
    }

    pub(in crate::ui) fn show_onboarding(&mut self, root: &mut egui::Ui) {
        let Some(mut wizard) = self.onboarding.take() else {
            return;
        };
        let action = wizard.show(
            root,
            Devices {
                inputs: &self.inputs,
                outputs: &self.outputs,
            },
        );
        let result = match action {
            Some(Action::Skip) => self.skip_onboarding(&wizard),
            Some(Action::Finish) => self.finish_onboarding(&wizard),
            Some(Action::BrowseFolder) => {
                match i18n::with_language(wizard.draft.language, || {
                    platform::choose_clip_folder(std::path::Path::new(wizard.folder.trim()))
                }) {
                    Ok(Some(folder)) => wizard.folder = folder.to_string_lossy().into_owned(),
                    Ok(None) => {}
                    Err(error) => wizard.error = error.to_string(),
                }
                Ok(false)
            }
            None => Ok(false),
        };
        match result {
            Ok(true) => {
                self.capturing_hotkey.store(false, Ordering::Relaxed);
                self.pressed_hotkeys.lock().unwrap().clear();
            }
            Ok(false) => self.onboarding = Some(wizard),
            Err(error) => {
                wizard.error = format!("{error:#}");
                self.onboarding = Some(wizard);
            }
        }
    }

    fn skip_onboarding(&mut self, wizard: &Wizard) -> Result<bool> {
        let saved = wizard.skipped_settings(&self.settings);
        saved.save()?;
        self.settings = saved.clone();
        self.refresh_tray_language();
        self.engine.send(Command::SetFeedback(
            saved.clip_feedback.clone(),
            saved.language,
        ))?;
        *self.active.lock().unwrap() = saved;
        Ok(true)
    }

    fn finish_onboarding(&mut self, wizard: &Wizard) -> Result<bool> {
        let candidate = wizard.candidate()?;
        platform::validate_clip_folder(&candidate.clips_dir)?;
        let previous = self.settings.clone();
        self.settings = candidate;
        if let Err(error) = self.apply_preferences(false) {
            self.settings = previous.clone();
            let _ = self.bind_hotkey();
            let _ = platform::startup(previous.start_with_windows);
            return Err(error);
        }
        self.storage_text = self.settings.clips_dir.to_string_lossy().into_owned();
        if self.settings.capture_enabled == Some(true) {
            self.engine
                .send(Command::Start(Box::new(self.settings.clone())))?;
        } else {
            self.engine.send(Command::Stop)?;
        }
        self.message = tr("Setup saved. Your moments stay on your PC.").into();
        Ok(true)
    }
}

pub(in crate::ui) fn open_preview() -> Result<()> {
    std::process::Command::new(std::env::current_exe()?)
        .arg("--onboarding-preview")
        .spawn()?;
    Ok(())
}
