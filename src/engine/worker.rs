//! Recording state machine. Command handling, source discovery and reporting
//! are separate steps so their transitions can be reviewed independently.
use super::saving::SaveQueue;
use super::*;

pub(super) struct Inputs {
    pub notify: Notify,
    pub force_dxgi: bool,
    pub shared: Arc<Mutex<Status>>,
    pub preview_request: Arc<AtomicBool>,
    pub commands: Receiver<Command>,
}

pub(super) struct Worker {
    pub(super) input: Inputs,
    pub(super) settings: Settings,
    armed: bool,
    pub(super) recording: Option<Recording>,
    feedback: Option<Feedback>,
    budget: Arc<Budget>,
    pub(super) saves: SaveQueue,
    pub(super) clip_id: u64,
    scan: Instant,
    report: Instant,
    retry_after: Instant,
}

pub(super) fn run(input: Inputs) {
    unsafe {
        let _ = windows::Win32::System::WinRT::RoInitialize(
            windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
        );
    }
    let feedback = match Feedback::new() {
        Ok(feedback) => Some(feedback),
        Err(error) => {
            input.shared.lock().unwrap().message = format!("Clip feedback unavailable: {error:#}");
            None
        }
    };
    let saves = SaveQueue::new(input.shared.clone(), input.notify.clone(), feedback.clone());
    let mut worker = Worker {
        input,
        feedback,
        saves,
        settings: Settings::default(),
        armed: false,
        recording: None,
        budget: Budget::new(MEMORY_LIMIT),
        clip_id: 0,
        scan: Instant::now() - Duration::from_secs(2),
        report: Instant::now(),
        retry_after: Instant::now(),
    };
    worker.run();
    worker.stop();
    worker.saves.finish();
    unsafe {
        windows::Win32::System::WinRT::RoUninitialize();
    }
}

impl Worker {
    fn run(&mut self) {
        let timer = crate::pacing::FrameTimer::new();
        loop {
            if let Some(rec) = &self.recording {
                let result = timer
                    .as_ref()
                    .map_err(|error| {
                        anyhow::anyhow!("Precise capture timing is unavailable: {error}")
                    })
                    .and_then(|timer| timer.until(rec.last_tick));
                if let Err(error) = result {
                    self.stop();
                    self.message(format!("Capture timing failed: {error}"));
                    continue;
                }
            }
            let command = if self.recording.is_some() {
                self.input.commands.try_recv().ok()
            } else {
                self.input
                    .commands
                    .recv_timeout(Duration::from_millis(50))
                    .ok()
            };
            if let Some(command) = command {
                if matches!(command, Command::Shutdown) {
                    break;
                }
                self.handle(command);
            }
            self.scan_source();
            self.tick();
            self.process_highlights();
            self.report();
        }
    }

    fn message(&self, message: String) {
        self.input.shared.lock().unwrap().message = message;
        self.input.notify.wake();
    }

    fn feedback(&self, event: FeedbackEvent) {
        if let Some(feedback) = &self.feedback {
            feedback.send(event);
        }
    }

    fn stop(&mut self) {
        self.flush_highlights(true);
        finish_recording(&mut self.recording, &self.input.shared);
        self.armed = false;
        self.input.shared.lock().unwrap().armed = false;
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::SetHighlights(preferences) => {
                self.flush_highlights(true);
                {
                    let mut state = self.input.shared.lock().unwrap();
                    state.highlight_game = None;
                    state.highlight_status.clear();
                    state.highlight_preview = None;
                }
                self.settings.highlights = preferences.clone();
                if let Some(rec) = &mut self.recording {
                    rec.highlights = None;
                    if let Target::Window(window) = &rec.target {
                        rec.highlights =
                            crate::highlights::Game::from_executable(&window.executable)
                                .filter(|game| {
                                    preferences
                                        .games
                                        .iter()
                                        .any(|g| g.game == *game && g.enabled)
                                })
                                .map(|game| {
                                    crate::highlights::Runtime::new(
                                        game,
                                        &preferences,
                                        rec.clock.clone(),
                                    )
                                });
                    }
                    rec.highlight_settings = preferences;
                    rec.buffer
                        .set_seconds(self.settings.replay_buffer_seconds().max(
                            if rec.highlights.is_some() {
                                crate::highlights::retention_seconds(
                                    rec.highlight_settings.before,
                                    rec.highlight_settings.after,
                                )
                            } else {
                                0
                            },
                        ));
                }
            }
            Command::SetReplayPreferences(seconds, hotkeys) => {
                self.settings.replay_seconds = seconds;
                self.settings.recording_hotkeys = hotkeys;
                if let Some(rec) = &mut self.recording {
                    rec.buffer
                        .set_seconds(self.settings.replay_buffer_seconds().max(
                            if rec.highlights.is_some() {
                                crate::highlights::retention_seconds(
                                    rec.highlight_settings.before,
                                    rec.highlight_settings.after,
                                )
                            } else {
                                0
                            },
                        ));
                }
            }
            Command::SetFeedback(preferences, language) => {
                self.settings.clip_feedback = preferences.clone();
                self.settings.language = language;
                self.feedback(FeedbackEvent::Configure(preferences, language));
            }
            Command::PreviewFeedback(preferences, language) => {
                self.feedback(FeedbackEvent::Preview(preferences, language))
            }
            Command::SetOutputFolder(folder) => {
                self.settings.clips_dir = folder;
                self.message("Folder updated. Future recordings and new clips use this folder; the current recording stays in its original folder.".into());
            }
            Command::Stop => {
                self.message("Clipping stopped.".into());
                self.stop();
            }
            Command::Start(settings) => self.start(*settings, true),
            Command::ToggleLongRecording(settings) => {
                let (settings, resume) =
                    long_recording_transition(*settings, self.settings.recording_mode, self.armed);
                self.start(settings, resume);
            }
            Command::Screenshot(preferences) => {
                if let Err(error) = self.screenshot(&preferences) {
                    self.feedback(FeedbackEvent::ScreenshotFinished {
                        source: String::new(),
                        error: Some(format!("{error:#}")),
                    });
                    self.message(format!("Screenshot failed: {error:#}"));
                }
            }
            Command::Save | Command::SaveFor(_) | Command::Bookmark => {
                let seconds = if let Command::SaveFor(seconds) = command {
                    seconds
                } else {
                    self.settings.replay_seconds
                };
                if let Err(error) = self.save_clip(seconds, matches!(command, Command::Bookmark)) {
                    self.feedback(FeedbackEvent::Failed(error.to_string()));
                    self.message(error.to_string());
                }
            }
            Command::Shutdown => unreachable!("shutdown is handled by the event loop"),
        }
        self.input.notify.wake();
    }

    fn start(&mut self, settings: Settings, resume: bool) {
        if settings.recording_mode == RecordingMode::Off {
            self.stop();
            self.message("Recording is off. Choose a recording mode in Settings.".into());
            return;
        }
        if let Err(error) = settings.validate() {
            self.message(error.to_string());
            return;
        }
        self.flush_highlights(true);
        finish_recording(&mut self.recording, &self.input.shared);
        self.settings = settings;
        self.feedback(FeedbackEvent::Configure(
            self.settings.clip_feedback.clone(),
            self.settings.language,
        ));
        if self.settings.recording_mode.automatic() {
            self.settings.mode = CaptureMode::Automatic;
        }
        self.armed = resume;
        self.scan = Instant::now() - Duration::from_secs(2);
        self.retry_after = Instant::now();
        let mut state = self.input.shared.lock().unwrap();
        state.armed = self.armed;
        state.recording = false;
        state.full_recording = self.armed && self.settings.recording_mode.full();
        state.elapsed = 0.0;
        state.buffered = 0.0;
        if self.armed {
            state.message = "Looking for the selected source…".into();
        }
    }

    fn screenshot(&mut self, preferences: &Settings) -> Result<()> {
        anyhow::ensure!(
            preferences.recording_mode != RecordingMode::Off,
            "Recording is off."
        );
        anyhow::ensure!(
            self.saves.pending() < 2,
            "Captures are still saving. Try again shortly."
        );
        let (image, source) = if let Some(rec) = &mut self.recording {
            anyhow::ensure!(
                rec.target.foreground(),
                "Bring the selected game or window to the foreground first."
            );
            let texture = rec
                .source
                .next()?
                .0
                .or_else(|| rec.source.latest())
                .context("Wait for the first frame before taking a screenshot.")?;
            (
                crate::screenshot::read_texture(
                    rec.source.device(),
                    rec.source.context(),
                    &texture,
                )?,
                rec.label.clone(),
            )
        } else {
            capture_screenshot(preferences, self.input.force_dxgi)?
        };
        self.saves.send(SaveJob::Screenshot {
            image,
            source,
            folder: preferences.clips_dir.clone(),
            limit: preferences.storage_limit_gb,
        })?;
        self.message("Saving screenshot…".into());
        Ok(())
    }

    fn save_clip(&mut self, seconds: u32, bookmark_only: bool) -> Result<()> {
        let rec = self
            .recording
            .as_ref()
            .context("No capture is active. Start capture first.")?;
        if let Some(session) = &rec.session {
            let seconds = session.bookmark(rec.clock.now())?;
            self.feedback(FeedbackEvent::Bookmarked {
                source: rec.label.clone(),
                seconds,
            });
            self.message(format!(
                "Bookmark saved at {}:{:02}",
                seconds as u64 / 60,
                seconds as u64 % 60
            ));
            return Ok(());
        }
        anyhow::ensure!(
            !bookmark_only,
            "Start a long recording before adding a bookmark."
        );
        anyhow::ensure!(
            self.saves.pending() < 2,
            "One clip is saving and another is queued. Try again shortly."
        );
        anyhow::ensure!(
            self.budget.used() < MEMORY_LIMIT - 4 * 1024 * 1024,
            "Replay memory is at its limit. Wait for the current save to finish."
        );
        let snapshot = rec.buffer.snapshot_for(&rec.label, seconds)?;
        crate::storage::check_limit(
            &self.settings.clips_dir,
            self.settings.storage_limit_gb,
            snapshot
                .packets
                .iter()
                .map(|packet| packet.data.bytes.len() as u64)
                .sum(),
        )?;
        self.clip_id = self.clip_id.wrapping_add(1).max(1);
        let source = snapshot.source.clone();
        let seconds = (snapshot.end - snapshot.start) as f64 / TICKS as f64;
        self.saves.send(SaveJob::Clip {
            id: self.clip_id,
            snapshot,
            folder: self.settings.clips_dir.clone(),
            events: Vec::new(),
        })?;
        self.feedback(FeedbackEvent::Captured {
            id: self.clip_id,
            source,
            seconds,
        });
        self.input.shared.lock().unwrap().saving = self.saves.pending();
        self.message("Saving clip in the background…".into());
        Ok(())
    }

    fn scan_source(&mut self) {
        if !self.armed
            || self.scan.elapsed() < Duration::from_millis(750)
            || Instant::now() < self.retry_after
        {
            return;
        }
        self.scan = Instant::now();
        if self.recording.as_ref().is_some_and(|rec| {
            !rec.target.exists()
                || (rec.target.foreground() && rec.fullscreen != rec.target.fullscreen())
        }) {
            self.finish_lost_source();
        }
        if !self.armed || self.recording.is_some() {
            return;
        }
        match sources::resolve(&self.settings).and_then(|target| {
            target
                .map(|target| {
                    Recording::new(
                        target,
                        &self.settings,
                        self.budget.clone(),
                        self.input.force_dxgi,
                    )
                })
                .transpose()
        }) {
            Ok(Some(rec)) => self.captured_source(rec),
            Ok(None) => self.message(
                if self.settings.mode == CaptureMode::Automatic {
                    "Waiting for a supported game…"
                } else {
                    "Waiting for the selected window to open…"
                }
                .into(),
            ),
            Err(error) => {
                self.retry_after = Instant::now() + Duration::from_secs(5);
                self.message(format!("Capture could not start: {error:#}"));
            }
        }
    }

    fn captured_source(&mut self, rec: Recording) {
        let mut state = self.input.shared.lock().unwrap();
        state.recording = true;
        state.full_recording = rec.session.is_some();
        state.source = rec.target.label();
        state.backend = rec.source.backend().into();
        state.message = if rec.session.is_some() {
            "Full recording is active. Stop & save when finished."
        } else {
            "Clipping is active."
        }
        .into();
        self.recording = Some(rec);
        drop(state);
        self.input.notify.wake();
    }

    fn finish_lost_source(&mut self) {
        self.flush_highlights(true);
        let full = self
            .recording
            .as_ref()
            .is_some_and(|rec| rec.session.is_some());
        finish_recording(&mut self.recording, &self.input.shared);
        if full && !self.settings.recording_mode.automatic() {
            self.armed = false;
            self.input.shared.lock().unwrap().armed = false;
            self.input.notify.wake();
        }
    }

    fn tick(&mut self) {
        let Some(rec) = &mut self.recording else {
            return;
        };
        match rec.tick(&self.input.preview_request, &self.input.shared) {
            Err(error) => {
                self.finish_lost_source();
                let previous = self.input.shared.lock().unwrap().message.clone();
                self.message(format!("Capture interrupted: {error:#}. {previous}"));
                self.retry_after = Instant::now() + Duration::from_secs(2);
            }
            Ok(()) => {
                if let Ok(warning) = rec.warnings.try_recv() {
                    self.message(warning);
                }
            }
        }
    }

    fn report(&mut self) {
        if self.report.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.report = Instant::now();
        let mut state = self.input.shared.lock().unwrap();
        state.memory = self.budget.used();
        state.saving = self.saves.pending();
        if let Some(rec) = &self.recording {
            state.elapsed = rec.clock.now() as f64 / TICKS as f64;
            state.buffered = rec.buffer.seconds();
            state.dropped = rec.video.dropped.load(Ordering::Relaxed);
            state.capture_fps = rec.capture_fps;
            state.fresh_fps = rec.fresh_fps;
        } else {
            state.capture_fps = 0.0;
            state.fresh_fps = 0.0;
        }
        drop(state);
        if self.armed {
            self.input.notify.wake();
        }
    }
}

fn capture_screenshot(
    preferences: &Settings,
    force_dxgi: bool,
) -> Result<(crate::screenshot::Screenshot, String)> {
    let target = sources::resolve(preferences)?
        .context("Open your game or select a capture source first.")?;
    anyhow::ensure!(
        target.foreground(),
        "Bring the selected game or window to the foreground first."
    );
    let label = target.recording_label(preferences);
    let mut source = CaptureSource::new(&target, force_dxgi)?;
    let started = Instant::now();
    let texture = loop {
        if let (Some(texture), _) = source.next()? {
            break texture;
        }
        anyhow::ensure!(
            started.elapsed() < Duration::from_millis(500),
            "The selected source did not produce a screenshot. Try again."
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok((
        crate::screenshot::read_texture(source.device(), source.context(), &texture)?,
        label,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker() -> Worker {
        let (_, commands) = bounded(1);
        let shared = Arc::new(Mutex::new(Status::default()));
        let notify = Notify {
            ctx: eframe::egui::Context::default(),
            visible: Arc::new(AtomicBool::new(false)),
        };
        Worker {
            saves: SaveQueue::new(shared.clone(), notify.clone(), None),
            input: Inputs {
                notify,
                force_dxgi: false,
                shared,
                commands,
                preview_request: Arc::new(AtomicBool::new(false)),
            },
            settings: Settings::default(),
            armed: false,
            recording: None,
            feedback: None,
            budget: Budget::new(MEMORY_LIMIT),
            clip_id: 0,
            scan: Instant::now(),
            report: Instant::now(),
            retry_after: Instant::now(),
        }
    }

    #[test]
    fn command_transitions_preserve_capture_preferences_and_do_not_start_hardware() {
        let mut worker = worker();
        worker.handle(Command::Start(Box::default()));
        assert!(worker.armed && worker.input.shared.lock().unwrap().armed);
        worker.handle(Command::ToggleLongRecording(Box::default()));
        assert_eq!(worker.settings.recording_mode, RecordingMode::FullRecording);
        worker.handle(Command::ToggleLongRecording(Box::default()));
        assert!(worker.armed);
        assert_eq!(worker.settings.recording_mode, RecordingMode::Clips);
        worker.handle(Command::Stop);
        assert!(!worker.input.shared.lock().unwrap().armed);
        worker.saves.finish();
    }

    #[test]
    fn invalid_start_keeps_existing_state_and_off_disarms() {
        let mut worker = worker();
        worker.start(Settings::default(), true);
        worker.start(
            Settings {
                fps: 500,
                ..Settings::default()
            },
            true,
        );
        assert!(worker.armed);
        assert_eq!(worker.settings.fps, Settings::default().fps);
        worker.start(
            Settings {
                recording_mode: RecordingMode::Off,
                ..Settings::default()
            },
            true,
        );
        assert!(!worker.armed);
        assert!(!worker.input.shared.lock().unwrap().armed);
        worker.saves.finish();
    }
}
