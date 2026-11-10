pub mod cs2;
mod model;
mod visual;
use crossbeam_channel::{Receiver, Sender, bounded};
pub use model::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
pub(super) type VisualBatch = Vec<(VisualRule, crate::screenshot::Screenshot, u64)>;
pub struct Runtime {
    pub game: Game,
    pub events: Receiver<GameEvent>,
    pub status: Arc<Mutex<String>>,
    frames: Option<Sender<VisualBatch>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub rules: Vec<VisualRule>,
    pub enabled: Vec<EventKind>,
}
impl Runtime {
    pub fn new(game: Game, settings: &HighlightSettings, clock: crate::audio::Clock) -> Self {
        let (tx, events) = bounded(32);
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new("Waiting for game events".into()));
        let configured = settings.games.iter().find(|g| g.game == game && g.enabled);
        let rules = configured.map(|g| g.rules.clone()).unwrap_or_default();
        let enabled = configured.map(|g| g.events.clone()).unwrap_or_default();
        let mut worker = None;
        let frames = if configured.is_none() {
            None
        } else if game == Game::Cs2 {
            match cs2::listen(settings, clock, tx, stop.clone(), status.clone()) {
                Ok(handle) => worker = Some(handle),
                Err(e) => *status.lock().unwrap() = e.to_string(),
            }
            None
        } else {
            let (sender, handle) = visual::start(game, tx, stop.clone(), status.clone());
            worker = Some(handle);
            Some(sender)
        };
        Self {
            game,
            events,
            status,
            frames,
            stop,
            worker,
            rules,
            enabled,
        }
    }
    pub fn wants_frame(&self) -> bool {
        self.frames.as_ref().is_some_and(|s| s.is_empty())
    }
    pub fn send_frames(&self, frames: VisualBatch) {
        if let Some(sender) = &self.frames {
            let _ = sender.try_send(frames);
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.frames.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
