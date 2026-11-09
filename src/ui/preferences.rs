use super::*;
impl Gavnel {
    pub(super) fn apply(&mut self) -> Result<()> {
        let previous = self.active.lock().unwrap().clone();
        if self.settings.recording_mode.automatic() {
            self.settings.mode = CaptureMode::Automatic;
        }
        self.settings.validate()?;
        if self.settings.clips_dir != previous.clips_dir {
            platform::validate_clip_folder(&self.settings.clips_dir)?;
        }
        self.bind_hotkey()?;
        if self.settings.start_with_windows != previous.start_with_windows {
            platform::startup(self.settings.start_with_windows)?;
        }
        self.settings.save()?;
        *self.active.lock().unwrap() = self.settings.clone();
        self.engine
            .send(Command::SetFeedback(self.settings.clip_feedback.clone()))?;
        self.engine.send(Command::SetReplayPreferences(
            self.settings.replay_seconds,
            self.settings.recording_hotkeys.clone(),
        ))?;
        self.library.refresh(&self.settings.clips_dir);
        if self.settings.recording_mode == RecordingMode::Off {
            self.engine.send(Command::Stop)?;
        } else if (self.engine.snapshot().armed && self.settings.recording_changed(&previous))
            || (self.settings.recording_mode.automatic()
                && self.settings.recording_mode != previous.recording_mode)
        {
            let mut runtime = self.settings.clone();
            if self.engine.snapshot().full_recording
                && runtime.recording_mode == RecordingMode::Clips
                && previous.recording_mode == RecordingMode::Clips
            {
                runtime.recording_mode = RecordingMode::FullRecording;
            }
            self.engine.send(Command::Start(Box::new(runtime)))?;
        } else if self.settings.clips_dir != previous.clips_dir {
            self.engine
                .send(Command::SetOutputFolder(self.settings.clips_dir.clone()))?;
        }
        Ok(())
    }
    pub(super) fn open_folder(&mut self) {
        let result = std::fs::create_dir_all(&self.settings.clips_dir)
            .map_err(anyhow::Error::from)
            .and_then(|_| platform::open(&self.settings.clips_dir));
        if let Err(e) = result {
            self.message = e.to_string();
        }
    }
    pub(super) fn browse_clip_folder(&mut self) {
        match platform::choose_clip_folder(&self.settings.clips_dir) {
            Ok(Some(path)) => {
                if let Err(e) = self.change_clip_folder(path) {
                    self.message = format!("{e:#}");
                }
            }
            Ok(None) => {}
            Err(e) => self.message = format!("Could not open the folder picker: {e:#}"),
        }
    }
    pub(super) fn change_clip_folder(&mut self, path: PathBuf) -> Result<()> {
        platform::validate_clip_folder(&path)?;
        let mut applied = self.active.lock().unwrap().clone();
        applied.clips_dir = path.clone();
        applied.save()?;
        self.engine.send(Command::SetOutputFolder(path.clone()))?;
        self.settings.clips_dir = path.clone();
        self.storage_text = path.to_string_lossy().into_owned();
        *self.active.lock().unwrap() = applied;
        self.textures.clear();
        self.library.refresh(&path);
        self.message = "Clip folder changed. Your replay buffer is still running.".into();
        Ok(())
    }
}
