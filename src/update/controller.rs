use super::*;
use std::sync::{Arc, atomic::AtomicU64, mpsc};
use std::time::{Duration, Instant};

pub enum State {
    Idle,
    Checking,
    Current,
    Available(Release),
    Downloading {
        received: Arc<AtomicU64>,
        total: u64,
    },
    Ready(Prepared),
    Failed(String),
}
pub struct Controller {
    pub state: State,
    pub installed: bool,
    receiver: Option<mpsc::Receiver<State>>,
    last_check: Option<Instant>,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            state: State::Idle,
            installed: super::installed(),
            receiver: None,
            last_check: None,
        }
    }
}
impl Controller {
    pub fn check(&mut self, ctx: &eframe::egui::Context) {
        if self.receiver.is_some() {
            return;
        }
        self.last_check = Some(Instant::now());
        self.start(State::Checking, ctx, || {
            metadata::latest().map(|release| release.map_or(State::Current, State::Available))
        });
    }
    pub fn poll(&mut self, ctx: &eframe::egui::Context, automatic: bool) {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(state) => {
                    self.state = state;
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.state =
                        State::Failed("The update worker stopped. Please try again.".into());
                    self.receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let due = self
            .last_check
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(6 * 60 * 60));
        if automatic
            && due
            && !matches!(
                self.state,
                State::Available(_) | State::Ready(_) | State::Downloading { .. }
            )
        {
            self.check(ctx);
        }
        if self.receiver.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        } else if automatic {
            ctx.request_repaint_after(Duration::from_secs(60 * 60));
        }
    }
    pub fn download(&mut self, release: Release, ctx: &eframe::egui::Context) {
        if self.receiver.is_some() {
            return;
        }
        let received = Arc::new(AtomicU64::new(0));
        let progress = received.clone();
        self.start(
            State::Downloading {
                received,
                total: release.size,
            },
            ctx,
            move || download::prepare(release, progress).map(State::Ready),
        );
    }
    pub fn restart(&self) -> Result<()> {
        let State::Ready(prepared) = &self.state else {
            anyhow::bail!("Download an update first");
        };
        install::start(prepared)
    }
    fn start(
        &mut self,
        state: State,
        ctx: &eframe::egui::Context,
        job: impl FnOnce() -> Result<State> + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("gavnel-updates".into())
            .spawn(move || {
                let result = job().unwrap_or_else(|error| State::Failed(format!("{error:#}")));
                let _ = sender.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.state = state;
                self.receiver = Some(receiver);
            }
            Err(error) => self.state = State::Failed(error.to_string()),
        }
    }
}
