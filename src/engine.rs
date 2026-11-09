mod recording;
mod saving;
mod worker;
use crate::{
    audio::{AudioWorker, Clock},
    config::{CaptureMode, ClipFeedback, RecordingMode, Settings},
    export::write_clip,
    feedback::{Event as FeedbackEvent, Feedback},
    replay::*,
    sources::{self, CaptureSource, Target},
    video::VideoEncoder,
};
use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, bounded};
use recording::{Recording, finish_recording};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub preview: Option<Arc<eframe::egui::ColorImage>>,
    pub armed: bool,
    pub recording: bool,
    pub full_recording: bool,
    pub elapsed: f64,
    pub source: String,
    pub backend: String,
    pub buffered: f64,
    pub memory: usize,
    pub dropped: u64,
    pub capture_fps: f64,
    pub fresh_fps: f64,
    pub saving: usize,
    pub message: String,
    pub last_clip: Option<PathBuf>,
    pub saved_count: u64,
}
pub enum Command {
    Start(Box<Settings>),
    ToggleLongRecording(Box<Settings>),
    Stop,
    Save,
    SaveFor(u32),
    Screenshot(Box<Settings>),
    Bookmark,
    SetOutputFolder(PathBuf),
    SetFeedback(ClipFeedback, crate::i18n::Language),
    SetReplayPreferences(u32, Option<Vec<crate::capture_hotkeys::CaptureHotkey>>),
    PreviewFeedback(ClipFeedback, crate::i18n::Language),
    Shutdown,
}
#[derive(Clone)]
pub struct Notify {
    pub ctx: eframe::egui::Context,
    pub visible: Arc<AtomicBool>,
}
impl Notify {
    pub fn wake(&self) {
        if self.visible.load(Ordering::Relaxed) {
            self.ctx.request_repaint();
        }
    }
}
pub struct Engine {
    pub preview_requested: Arc<AtomicBool>,
    pub commands: Sender<Command>,
    pub status: Arc<Mutex<Status>>,
    worker: Option<JoinHandle<()>>,
}
enum SaveJob {
    Clip {
        id: u64,
        snapshot: Snapshot,
        folder: PathBuf,
    },
    Screenshot {
        image: crate::screenshot::Screenshot,
        folder: PathBuf,
        source: String,
        limit: Option<u32>,
    },
}

/// Long recording is a runtime action; the three saved modes stay unchanged.
fn long_recording_transition(
    mut preferences: Settings,
    current: RecordingMode,
    armed: bool,
) -> (Settings, bool) {
    if preferences.recording_mode == RecordingMode::Off {
        return (preferences, false);
    }
    let stopping = armed && current.full();
    if !stopping {
        if preferences.recording_mode.automatic() {
            preferences.mode = CaptureMode::Automatic;
        }
        preferences.recording_mode = RecordingMode::FullRecording;
    }
    // Ending an automatic session pauses auto capture until explicitly enabled
    // again, rather than immediately starting another full recording.
    let resume = !stopping || !preferences.recording_mode.automatic();
    (preferences, resume)
}

impl Engine {
    pub fn new(notify: Notify) -> Self {
        Self::with_backend(notify, false)
    }
    pub fn with_backend(notify: Notify, force_dxgi: bool) -> Self {
        let (commands, rx) = bounded(16);
        let status = Arc::new(Mutex::new(Status {
            message: "Choose a source and start clipping.".into(),
            ..Default::default()
        }));
        let preview_requested = Arc::new(AtomicBool::new(false));
        let preview_request = preview_requested.clone();
        let shared = status.clone();
        let worker = std::thread::Builder::new()
            .name("gavnel-engine".into())
            .spawn(move || {
                worker::run(worker::Inputs {
                    notify,
                    force_dxgi,
                    shared,
                    preview_request,
                    commands: rx,
                })
            })
            .expect("Could not start recording engine");
        Self {
            preview_requested,
            commands,
            status,
            worker: Some(worker),
        }
    }
    pub fn send(&self, command: Command) -> Result<()> {
        self.commands
            .try_send(command)
            .context("Gavnel is busy; try again in a moment")
    }
    pub fn snapshot(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[cfg(test)]
mod long_recording_tests {
    use super::*;
    #[test]
    fn long_shortcut_starts_and_stops_without_changing_saved_manual_mode() {
        let preferences = Settings::default();
        for was_armed in [false, true] {
            let (running, armed) =
                long_recording_transition(preferences.clone(), RecordingMode::Clips, was_armed);
            assert!(armed);
            assert_eq!(running.recording_mode, RecordingMode::FullRecording);
            let (resumed, armed) =
                long_recording_transition(preferences.clone(), running.recording_mode, true);
            assert!(armed);
            assert_eq!(resumed.recording_mode, RecordingMode::Clips);
            assert_eq!(preferences.recording_mode, RecordingMode::Clips);
        }
    }
    #[test]
    fn recording_off_blocks_long_recording_and_auto_stop_stays_stopped() {
        let mut preferences = Settings {
            recording_mode: RecordingMode::Off,
            ..Settings::default()
        };
        let (settings, armed) =
            long_recording_transition(preferences.clone(), RecordingMode::Clips, false);
        assert!(!armed);
        assert_eq!(settings.recording_mode, RecordingMode::Off);
        preferences.recording_mode = RecordingMode::AutomaticSessions;
        let (settings, armed) =
            long_recording_transition(preferences, RecordingMode::AutomaticSessions, true);
        assert!(!armed);
        assert_eq!(settings.recording_mode, RecordingMode::AutomaticSessions);
    }
}
