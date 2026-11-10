//! Active capture, encoder submission, and recording finalization.
use super::*;
pub(super) struct Recording {
    pub(super) preview_at: Instant,
    pub(super) label: String,
    pub(super) source: CaptureSource,
    pub(super) target: Target,
    pub(super) video: VideoEncoder,
    pub(super) _audio: Option<AudioWorker>,
    pub(super) clock: Clock,
    pub(super) buffer: ReplayBuffer,
    pub(super) packets: Receiver<Packet>,
    pub(super) fatal: Receiver<String>,
    pub(super) warnings: Receiver<String>,
    pub(super) last_tick: Instant,
    pub(super) interval: Duration,
    pub(super) fullscreen: bool,
    pub(super) rate_since: Instant,
    pub(super) submitted: u64,
    pub(super) fresh: u64,
    pub(super) capture_fps: f64,
    pub(super) fresh_fps: f64,
    pub(super) session: Option<crate::recording::SessionWriter>,
    pub(super) highlights: Option<crate::highlights::Runtime>,
    pub(super) pending_highlights: Vec<crate::highlights::PendingWindow>,
    pub(super) highlight_settings: crate::highlights::HighlightSettings,
    pub(super) highlight_at: Instant,
    pub(super) calibration_at: Instant,
}
impl Recording {
    pub(super) fn new(
        target: Target,
        settings: &Settings,
        budget: Arc<Budget>,
        force_dxgi: bool,
    ) -> Result<Self> {
        let mut audio_settings = settings.clone();
        if let Target::Window(window) = &target {
            audio_settings.game_audio_executable = window.executable.clone();
        }
        let source = CaptureSource::new(&target, force_dxgi)?;
        let (tx, packets) = bounded(256);
        let (error_tx, fatal) = bounded(4);
        let (warning_tx, warnings) = bounded(8);
        let video = VideoEncoder::new(
            source.device().clone(),
            source.context().clone(),
            settings,
            crate::replay::EncoderOutput {
                budget: budget.clone(),
                packets: tx.clone(),
                messages: error_tx,
            },
        )?;
        let clock = Clock::new()?;
        let has_audio = settings.desktop_audio || settings.microphone;
        let audio_names = audio_settings.application_names();
        let audio = if has_audio {
            Some(AudioWorker::start(
                audio_settings,
                clock.clone(),
                crate::replay::EncoderOutput {
                    budget,
                    packets: tx,
                    messages: warning_tx,
                },
            )?)
        } else {
            None
        };
        let mut buffer = ReplayBuffer::new(settings.replay_buffer_seconds(), has_audio);
        buffer.video = Some(video.config.clone());
        buffer.audio_names = audio_names.clone();
        let fullscreen = target.fullscreen();
        let label = target.recording_label(settings);
        let session = if settings.recording_mode.full() {
            Some(crate::recording::SessionWriter::start_configured(
                &label,
                &video.config,
                settings,
                audio_names,
            )?)
        } else {
            None
        };
        let highlights = if let Target::Window(window) = &target {
            crate::highlights::Game::from_executable(&window.executable)
                .filter(|game| {
                    settings
                        .highlights
                        .games
                        .iter()
                        .any(|g| g.game == *game && g.enabled)
                })
                .map(|game| {
                    crate::highlights::Runtime::new(game, &settings.highlights, clock.clone())
                })
        } else {
            None
        };
        if highlights.is_some() {
            buffer.set_seconds(settings.replay_buffer_seconds().max(
                crate::highlights::retention_seconds(
                    settings.highlights.before,
                    settings.highlights.after,
                ),
            ));
        }
        Ok(Self {
            highlights,
            pending_highlights: Vec::new(),
            highlight_settings: settings.highlights.clone(),
            highlight_at: Instant::now(),
            calibration_at: Instant::now(),
            label,
            source,
            target,
            video,
            _audio: audio,
            clock,
            buffer,
            packets,
            fatal,
            warnings,
            preview_at: Instant::now(),
            last_tick: Instant::now(),
            interval: Duration::from_secs_f64(1.0 / settings.fps as f64),
            fullscreen,
            rate_since: Instant::now(),
            submitted: 0,
            fresh: 0,
            capture_fps: 0.0,
            fresh_fps: 0.0,
            session,
        })
    }
    fn drain(&mut self) -> Result<()> {
        for p in self.packets.try_iter() {
            if let Some(session) = &self.session {
                session.push(p.clone())?;
            }
            self.buffer.push(p);
        }
        if self.session.is_some() {
            self.buffer.trim_bytes(MEMORY_LIMIT - 128 * 1024 * 1024);
        }
        Ok(())
    }
    pub(super) fn finish(mut self) -> Result<Option<PathBuf>> {
        // Join producers first, then forward their final packets before closing the writer.
        drop(self._audio.take());
        drop(self.video);
        let mut forwarding_error = None;
        for packet in self.packets.try_iter() {
            if let Some(session) = &self.session
                && let Err(error) = session.push_final(packet)
            {
                forwarding_error = Some(error);
                break;
            }
        }
        let path = self.session.map(|session| session.finish()).transpose()?;
        if let Some(error) = forwarding_error {
            anyhow::bail!(
                "{error:#}. Saved received frames to {}",
                path.as_deref()
                    .unwrap_or_else(|| std::path::Path::new("(no video)"))
                    .display()
            );
        }
        Ok(path)
    }
    pub(super) fn tick(&mut self, requested: &AtomicBool, shared: &Mutex<Status>) -> Result<()> {
        if let Ok(error) = self.fatal.try_recv() {
            anyhow::bail!("{error}");
        }
        self.drain()?;
        let now = Instant::now();
        if now < self.last_tick {
            return Ok(());
        }
        let skipped = crate::pacing::advance(&mut self.last_tick, now, self.interval);
        self.video.dropped.fetch_add(skipped, Ordering::Relaxed);
        let foreground = self.target.foreground();
        let (frame, fresh) = if foreground {
            self.source.next()?
        } else {
            (None, false)
        };
        if frame.is_none() {
            shared.lock().unwrap().preview = None;
        }
        // Never retain the previous game image after focus loss. The encoder
        // receives a reusable black GPU texture while audio stays continuous.
        if self.video.submit(frame.as_ref(), self.clock.now())? {
            self.submitted += 1;
        }
        // Read the actual captured frame only when the source card requests it.
        // A preview failure must never interrupt recording.
        if now >= self.preview_at && requested.swap(false, Ordering::Relaxed) {
            self.preview_at = now + Duration::from_millis(500);
            let preview = frame.as_ref().and_then(|texture| {
                crate::screenshot::read_texture(
                    self.source.device(),
                    self.source.context(),
                    texture,
                )
                .ok()
                .map(|image| Arc::new(image.preview()))
            });
            shared.lock().unwrap().preview = preview;
        }
        if now >= self.highlight_at {
            self.highlight_at = now + Duration::from_millis(400);
            if let Some(highlights) = &self.highlights {
                shared.lock().unwrap().highlight_game = Some(highlights.game);
                shared.lock().unwrap().highlight_status = highlights.status.lock().unwrap().clone();
                if foreground
                    && now >= self.calibration_at
                    && let Some(texture) = &frame
                {
                    self.calibration_at = now + Duration::from_secs(2);
                    if let Ok(image) = crate::screenshot::read_texture(
                        self.source.device(),
                        self.source.context(),
                        texture,
                    ) {
                        shared.lock().unwrap().highlight_preview = Some(Arc::new(image.preview()));
                    }
                }
                if foreground
                    && highlights.wants_frame()
                    && let Some(texture) = &frame
                {
                    let frames = highlights
                        .rules
                        .iter()
                        .filter(|r| highlights.enabled.contains(&r.kind))
                        .filter_map(|rule| {
                            crate::screenshot::read_region(
                                self.source.device(),
                                self.source.context(),
                                texture,
                                rule.region,
                            )
                            .ok()
                            .map(|image| (rule.clone(), image, self.clock.now()))
                        })
                        .collect();
                    highlights.send_frames(frames);
                }
            }
        }
        self.fresh += u64::from(fresh);
        if self.rate_since.elapsed() >= Duration::from_secs(1) {
            let seconds = self.rate_since.elapsed().as_secs_f64();
            self.capture_fps = self.submitted as f64 / seconds;
            self.fresh_fps = self.fresh as f64 / seconds;
            self.submitted = 0;
            self.fresh = 0;
            self.rate_since = Instant::now();
        }
        Ok(())
    }
}

pub(super) fn finish_recording(recording: &mut Option<Recording>, shared: &Mutex<Status>) {
    let rec = recording.take();
    {
        let mut state = shared.lock().unwrap();
        state.highlight_game = None;
        state.highlight_preview = None;
        state.highlight_status.clear();
        state.recording = false;
        state.full_recording = false;
        state.elapsed = 0.0;
        state.buffered = 0.0;
        state.source.clear();
        state.preview = None;
        state.backend.clear();
    }
    let Some(rec) = rec else {
        return;
    };
    match rec.finish() {
        Ok(Some(path)) => {
            let mut state = shared.lock().unwrap();
            state.message = format!(
                "Saved recording: {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            state.last_clip = Some(path);
            state.saved_count += 1;
        }
        Ok(None) => {}
        Err(error) => shared.lock().unwrap().message = format!("Recording stopped: {error:#}"),
    }
}
