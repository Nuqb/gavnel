mod body;
mod buttons;
mod controls;
mod crop_canvas;
mod header;
mod navigation;
mod save;
mod save_dialog;
#[cfg(test)]
mod tests;
mod timeline;
mod transport;
use super::*;
use crate::media::{EditProject, ExportOptions, JobProgress, MediaReply, MediaService};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Trim,
    Crop,
    Audio,
}
pub(super) struct Editor {
    pub project: Option<EditProject>,
    pub loading: bool,
    saved_project: Option<EditProject>,
    leave: Option<navigation::Destination>,
    return_player: Option<PathBuf>,
    id: u64,
    frame_id: u64,
    selected: usize,
    tab: Tab,
    undo: Vec<EditProject>,
    redo: Vec<EditProject>,
    pending: Option<EditProject>,
    texture: Option<egui::TextureHandle>,
    cursor: f64,
    preview_origin: f64,
    play_requested: bool,
    rendered_playback: bool,
    error: String,
    save_open: bool,
    saving_overlay: bool,
    saving_video: Option<Arc<Mutex<JobProgress>>>,
    folder: PathBuf,
    filename: String,
    height: Option<u32>,
    size_mb: Option<f64>,
    pub(in crate::ui) pending_save: Option<super::saving::Job>,
    preview_job: Option<(Arc<Mutex<JobProgress>>, Arc<AtomicBool>)>,
    save_editing: bool,
    last_frame: Option<(PathBuf, u64)>,
    frame_pending: bool,
    filmstrips: HashMap<PathBuf, Vec<egui::TextureHandle>>,
    strip_pending: Option<(u64, PathBuf)>,
    strip_serial: u64,
    crop_drag: Option<crop_canvas::Drag>,
}
impl Editor {
    fn new(folder: PathBuf) -> Self {
        Self {
            project: None,
            loading: true,
            saved_project: None,
            leave: None,
            return_player: None,
            id: 1,
            frame_id: 0,
            selected: 0,
            tab: Tab::Trim,
            undo: vec![],
            redo: vec![],
            pending: None,
            texture: None,
            cursor: 0.0,
            preview_origin: 0.0,
            play_requested: false,
            rendered_playback: false,
            error: String::new(),
            save_open: false,
            saving_overlay: false,
            saving_video: None,
            folder,
            filename: "Edited clip.mp4".into(),
            height: None,
            size_mb: None,
            pending_save: None,
            preview_job: None,
            save_editing: false,
            last_frame: None,
            frame_pending: false,
            filmstrips: HashMap::new(),
            strip_pending: None,
            strip_serial: 0,
            crop_drag: None,
        }
    }
    fn poll(&mut self, ui: &mut egui::Ui, service: &MediaService) {
        for reply in service.replies.try_iter() {
            match reply {
                MediaReply::Open(id, result) if id == self.id => {
                    self.loading = false;
                    match result {
                        Ok(project) => {
                            self.saved_project = Some(project.clone());
                            self.project = Some(project);
                        }
                        Err(error) => self.error = error,
                    }
                }
                MediaReply::Filmstrip(id, result)
                    if self
                        .strip_pending
                        .as_ref()
                        .is_some_and(|(pending, _)| *pending == id) =>
                {
                    self.receive_filmstrip(ui.ctx(), id, result);
                }
                MediaReply::Frame(id, result) if id == self.frame_id => {
                    self.frame_pending = false;
                    match result {
                        Ok(image) => {
                            self.texture = Some(ui.ctx().load_texture(
                                "editor-frame",
                                image,
                                egui::TextureOptions::LINEAR,
                            ))
                        }
                        Err(error) => self.error = error,
                    }
                }
                _ => {}
            }
        }
        if self.loading
            || self.frame_pending
            || self.strip_pending.is_some()
            || self
                .preview_job
                .as_ref()
                .is_some_and(|(p, _)| p.lock().unwrap().result.is_none())
        {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }
    fn request_frame(&mut self, service: &MediaService) {
        let Some(s) = self
            .project
            .as_ref()
            .and_then(|p| p.segments.get(self.selected))
        else {
            return;
        };
        let time = self
            .cursor
            .clamp(s.start, (s.end - 1.0 / s.info.fps).max(s.start));
        let stamp = (s.info.path.clone(), (time * 1000.0) as u64);
        if self.last_frame.as_ref() == Some(&stamp) {
            return;
        }
        self.frame_id = self.frame_id.wrapping_add(1);
        if service
            .frame(self.frame_id, s.info.path.clone(), time)
            .is_ok()
        {
            self.last_frame = Some(stamp);
            self.frame_pending = true;
        }
    }
    fn record_edit(&mut self, before: EditProject, down: bool) {
        if self.project.as_ref() != Some(&before) && self.pending.is_none() {
            self.pending = Some(before);
            self.redo.clear();
        }
        if !down && let Some(previous) = self.pending.take() {
            if self.undo.len() >= 100 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
        }
    }
    fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            if let Some(current) = self.project.replace(p) {
                self.redo.push(current);
            }
            self.selected = 0;
            self.last_frame = None;
        }
    }
    fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            if let Some(current) = self.project.replace(p) {
                self.undo.push(current);
            }
            self.selected = 0;
            self.last_frame = None;
        }
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        if let Some((_, c)) = &self.preview_job {
            c.store(true, Ordering::Release);
        }
    }
}
impl Gavnel {
    pub(super) fn begin_editor(&mut self, paths: Vec<PathBuf>) {
        if let Some(path) = paths.iter().find(|path| self.saves.status(path).is_some()) {
            self.saves.show_for(path);
            return;
        }
        let return_player = if self.page == Page::Player {
            self.player.as_ref().map(|p| p.path.clone())
        } else {
            None
        };
        self.player = None;
        let mut editor = Editor::new(self.settings.clips_dir.clone());
        editor.return_player = return_player;
        editor.id = self.started.elapsed().as_nanos() as u64;
        editor.frame_id = editor.id;
        if let Err(error) = self.media.open(editor.id, paths) {
            editor.error = error.to_string();
            editor.loading = false;
        }
        self.editor = Some(editor);
        self.page = Page::Editor;
        self.quick_open = false;
        self.clips_open = false;
        self.long_recording_menu_open = false;
    }
    pub(super) fn edit_view(&mut self, ui: &mut egui::Ui) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        editor.saving_overlay = self.saves.modal.is_some();
        editor.poll_save(&self.media);
        editor.poll(ui, &self.media);
        ui.add_enabled_ui(editor.saving_video.is_none(), |ui| {
            editor.poll_playback(
                &mut self.player,
                transport::Host {
                    parent: self.parent_window,
                    context: &self.context,
                    visible: &self.visible,
                },
            );
            editor.header(ui, self);
            if editor.leave.is_none() && !editor.save_open && !ui.ctx().egui_wants_keyboard_input()
            {
                let (undo, redo) = ui.input_mut(|i| {
                    (
                        i.consume_key(egui::Modifiers::CTRL, egui::Key::Z),
                        i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z),
                    )
                });
                if undo {
                    editor.undo();
                }
                if redo {
                    editor.redo();
                }
            }
            if editor.loading {
                ui.spinner();
                ui.label(tr("Opening clips…"));
            }
            editor.update_cursor(&mut self.player);
            editor.body(ui, &self.media, &mut self.player);
            if editor.play_requested {
                editor.play_requested = false;
                editor.toggle_playback(
                    &self.media,
                    &mut self.player,
                    transport::Host {
                        parent: self.parent_window,
                        context: &self.context,
                        visible: &self.visible,
                    },
                );
            }
            if self
                .player
                .as_ref()
                .is_some_and(|p| !p.paused() && !p.ended())
            {
                ui.ctx().request_repaint_after(Duration::from_millis(16));
            }
            if editor.preview_job.is_some() {
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
            if !editor.error.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(DANGER, &editor.error);
                    if ui.small_button(tr("Dismiss")).clicked() {
                        editor.error.clear();
                    }
                });
            }
            editor.save_video_dialog(ui, &self.media);
        });
        self.editor = Some(editor);
        self.collect_video_save();
    }
}
