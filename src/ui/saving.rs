//! Save jobs live with the app, independently of editor navigation and lifetime.
use super::*;
use crate::media::JobProgress;

pub(super) struct Job {
    pub sources: Vec<PathBuf>,
    pub target: PathBuf,
    pub replacing: bool,
    pub state: Arc<Mutex<JobProgress>>,
    pub cancel: Arc<AtomicBool>,
    id: u64,
}
impl Job {
    pub fn new(
        project: &crate::media::EditProject,
        options: &crate::media::ExportOptions,
        handles: (Arc<Mutex<JobProgress>>, Arc<AtomicBool>),
    ) -> Self {
        Self {
            sources: project
                .segments
                .iter()
                .map(|s| s.info.path.clone())
                .collect(),
            target: options.path.clone(),
            replacing: options.replace_source,
            state: handles.0,
            cancel: handles.1,
            id: 0,
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn affects(&self, path: &std::path::Path) -> bool {
        self.target == path || self.sources.iter().any(|source| source == path)
    }
}

pub(super) struct Notice {
    pub result: Result<PathBuf, String>,
}

#[derive(Default)]
pub(super) struct Saves {
    pub jobs: Vec<Job>,
    pub notices: Vec<Notice>,
    pub modal: Option<u64>,
    serial: u64,
}
impl Saves {
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }
    pub fn push(&mut self, mut job: Job) {
        self.serial += 1;
        job.id = self.serial;
        self.modal = Some(job.id);
        self.jobs.push(job);
    }
    pub fn status(&self, path: &std::path::Path) -> Option<JobProgress> {
        self.jobs
            .iter()
            .find(|job| job.affects(path))
            .map(|job| job.state.lock().unwrap().clone())
    }
    pub fn replacing(&self, path: &std::path::Path) -> bool {
        self.jobs
            .iter()
            .any(|job| job.replacing && job.target == path)
    }
    pub fn show_for(&mut self, path: &std::path::Path) {
        self.modal = self
            .jobs
            .iter()
            .find(|job| job.affects(path))
            .map(|job| job.id);
    }
    pub fn poll(&mut self) -> Vec<PathBuf> {
        let mut changed = vec![];
        self.jobs.retain(|job| {
            let Some(result) = job.state.lock().unwrap().result.clone() else {
                return true;
            };
            if let Ok(path) = &result {
                changed.push(path.clone());
            }
            if self.modal == Some(job.id) {
                self.modal = None;
            }
            self.notices.push(Notice { result });
            false
        });
        changed
    }
}

impl Gavnel {
    pub(super) fn collect_video_save(&mut self) {
        if let Some(job) = self
            .editor
            .as_mut()
            .and_then(|editor| editor.pending_save.take())
        {
            self.window_actions.set_exit_blocked(true);
            self.saves.push(job);
        }
    }
    pub(super) fn poll_video_saves(&mut self, ctx: &egui::Context) {
        self.collect_video_save();
        if let Some(editor) = &mut self.editor {
            editor.poll_save(&self.media);
        }
        for path in self.saves.poll() {
            self.textures.remove(&path);
            self.thumbnails.invalidate(&path);
            self.library.refresh(&self.settings.clips_dir);
        }
        self.window_actions.set_exit_blocked(self.saves.busy());
        if self.saves.busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub(super) fn protect_video_saves(&mut self, ctx: &egui::Context) -> bool {
        let blocked = self.saves.busy()
            && (self.quitting.load(Ordering::Acquire)
                || ctx.input(|i| i.viewport().close_requested()));
        if blocked {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.quitting.store(false, Ordering::Release);
            self.saves.modal = self.saves.jobs.first().map(Job::id);
            self.window_actions.show();
        }
        blocked
    }

    pub(super) fn video_save_activity(&mut self, ui: &mut egui::Ui) {
        let Some(job) = self.saves.jobs.first() else {
            return;
        };
        let progress = job.state.lock().unwrap().clone();
        let id = job.id;
        ui.horizontal(|ui| {
            ui.label(tr("Saving video"));
            ui.add(
                egui::ProgressBar::new(progress.fraction)
                    .desired_width(180.0)
                    .show_percentage(),
            );
            if ui.small_button(tr("Show progress")).clicked() {
                self.saves.modal = Some(id);
            }
        });
        ui.add_space(12.0);
    }

    pub(super) fn video_save_ui(&mut self, ctx: &egui::Context) {
        self.collect_video_save();
        let action = progress_modal(ctx, &mut self.saves);
        if action == ProgressAction::Library {
            if let Some(editor) = &mut self.editor {
                editor.clear_pending_leave();
            }
            self.saves.modal = None;
            self.page = Page::Clips;
            self.player = None;
        }
        let view = completion_banner(ctx, &mut self.saves.notices);
        self.clip_save_overlays(ctx);
        if let Some(path) = view {
            if self.page == Page::Editor
                && self
                    .editor
                    .as_mut()
                    .is_some_and(|editor| editor.defer_player(path.clone()))
            {
                self.player = None;
            } else {
                self.open_player(&path);
            }
        }
    }
    fn clip_save_overlays(&self, ctx: &egui::Context) {
        let Some(player) = &self.player else {
            return;
        };
        let Some(rect) =
            ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("video-layout-rect")))
        else {
            return;
        };
        if self.saves.modal.is_some() {
            let _ = player.layout(rect, ctx.pixels_per_point(), false);
            return;
        }
        let popup =
            ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("video-popup-rect")));
        let banner = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("video-save-banner-rect")));
        let overlay = match (popup, banner) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        };
        player.set_occlusion(rect, ctx.pixels_per_point(), overlay);
    }
}

#[derive(PartialEq)]
enum ProgressAction {
    None,
    Library,
}

fn progress_modal(ctx: &egui::Context, saves: &mut Saves) -> ProgressAction {
    let Some(job) = saves.jobs.iter().find(|job| Some(job.id) == saves.modal) else {
        return ProgressAction::None;
    };
    let progress = job.state.lock().unwrap().clone();
    let mut hide = false;
    let response = egui::Modal::new(egui::Id::unique("saving-video-progress"))
        .backdrop_color(Color32::from_black_alpha(180))
        .frame(modal_frame())
        .show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label(RichText::new(tr("Saving video")).size(24.0).strong());
            ui.add_space(14.0);
            ui.add(
                egui::Label::new(job.target.file_name().unwrap_or_default().to_string_lossy())
                    .truncate(),
            );
            ui.add_space(12.0);
            ui.add(
                egui::ProgressBar::new(progress.fraction)
                    .desired_width(ui.available_width())
                    .show_percentage(),
            );
            ui.label(RichText::new(tr(&progress.message)).color(MUTED).size(12.0));
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let library = views::orange_button(ui, tr("Back to library")).clicked();
                if ui.button(tr("Continue in background")).clicked() {
                    hide = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    job.cancel.store(true, Ordering::Release);
                }
                if library {
                    ProgressAction::Library
                } else {
                    ProgressAction::None
                }
            })
            .inner
        });
    if hide || response.should_close() {
        saves.modal = None;
    }
    response.inner
}

fn modal_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(16.0)
        .inner_margin(26.0)
        .shadow(egui::epaint::Shadow {
            offset: [0, 14],
            blur: 36,
            spread: 0,
            color: Color32::from_black_alpha(110),
        })
}

fn completion_banner(ctx: &egui::Context, notices: &mut Vec<Notice>) -> Option<PathBuf> {
    ctx.data_mut(|data| data.remove::<egui::Rect>(egui::Id::unique("video-save-banner-rect")));
    let notice = notices.first()?;
    let mut view = None;
    let mut dismiss = false;
    let response = egui::Area::new(egui::Id::unique("video-saved-banner"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-24.0, -24.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(PANEL)
                .stroke(egui::Stroke::new(1.0, LINE))
                .corner_radius(12.0)
                .inner_margin(18.0)
                .show(ui, |ui| {
                    ui.set_max_width(420.0);
                    ui.horizontal(|ui| {
                        match &notice.result {
                            Ok(path) => {
                                ui.label(RichText::new(tr("Video saved")).strong());
                                if views::orange_button(ui, tr("View")).clicked() {
                                    view = Some(path.clone());
                                }
                            }
                            Err(error) => {
                                ui.colored_label(DANGER, error);
                            }
                        }
                        dismiss = ui.button(tr("Dismiss")).clicked();
                    });
                    if let Ok(path) = &notice.result {
                        ui.add(
                            egui::Label::new(
                                path.file_name().unwrap_or_default().to_string_lossy(),
                            )
                            .truncate(),
                        );
                    }
                });
        });
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::unique("video-save-banner-rect"),
            response.response.rect.expand(2.0),
        )
    });
    if dismiss || view.is_some() {
        notices.remove(0);
    }
    view
}

#[cfg(test)]
#[path = "saving_tests.rs"]
mod tests;
