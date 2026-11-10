mod body;
mod buttons;
mod controls;
mod crop_canvas;
mod navigation;
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
    export_open: bool,
    folder: PathBuf,
    filename: String,
    height: Option<u32>,
    size_mb: Option<f64>,
    jobs: Vec<(Arc<Mutex<JobProgress>>, Arc<AtomicBool>)>,
    preview_job: Option<(Arc<Mutex<JobProgress>>, Arc<AtomicBool>)>,
    project_path: String,
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
            export_open: false,
            folder,
            filename: "Edited clip.mp4".into(),
            height: None,
            size_mb: None,
            jobs: vec![],
            preview_job: None,
            project_path: String::new(),
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
                    let (_, path) = self.strip_pending.take().unwrap();
                    if let Ok(images) = result {
                        let textures = images
                            .into_iter()
                            .enumerate()
                            .map(|(i, image)| {
                                ui.ctx().load_texture(
                                    format!("timeline-{id}-{i}"),
                                    image,
                                    egui::TextureOptions::LINEAR,
                                )
                            })
                            .collect();
                        self.filmstrips.insert(path, textures);
                    }
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
                .jobs
                .iter()
                .any(|(p, _)| p.lock().unwrap().result.is_none())
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
    fn save_project(&mut self) -> bool {
        if let Some(p) = &self.project {
            let path = PathBuf::from(&self.project_path);
            match p.save(&path) {
                Ok(()) => {
                    self.saved_project = Some(p.clone());
                    self.error.clear();
                    return true;
                }
                Err(e) => self.error = e.to_string(),
            }
        }
        false
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        for (_, c) in &self.jobs {
            c.store(true, Ordering::Release);
        }
        if let Some((_, c)) = &self.preview_job {
            c.store(true, Ordering::Release);
        }
    }
}
impl Gavnel {
    pub(super) fn begin_editor(&mut self, paths: Vec<PathBuf>) {
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
        let projects = Settings::path()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("Projects");
        if let Err(error) = std::fs::create_dir_all(&projects) {
            self.message = error.to_string();
            return;
        }
        editor.project_path = projects
            .join(format!(
                "Edit-{}.gavnel-project",
                chrono::Local::now().format("%Y%m%d-%H%M%S-%3f")
            ))
            .to_string_lossy()
            .into_owned();
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
        editor.poll(ui, &self.media);
        editor.poll_playback(
            &mut self.player,
            transport::Host {
                parent: self.parent_window,
                context: &self.context,
                visible: &self.visible,
            },
        );
        ui.horizontal(|ui| {
            if ui
                .button(tr(if editor.return_player.is_some() {
                    "‹  Player"
                } else {
                    "‹  Library"
                }))
                .clicked()
            {
                self.page = if editor.return_player.is_some() {
                    Page::Player
                } else {
                    Page::Clips
                };
                self.player = None;
            }
            ui.label(RichText::new(tr("Editor")).size(24.0).strong());
            if let Some(segment) = editor
                .project
                .as_ref()
                .and_then(|p| p.segments.get(editor.selected))
            {
                let name = segment
                    .info
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let name_width = (ui.available_width() - 260.0).clamp(0.0, 320.0);
                ui.add_sized(
                    [name_width, 24.0],
                    egui::Label::new(RichText::new(name.as_ref()).size(12.0).color(MUTED))
                        .truncate(),
                )
                .on_hover_text(name.as_ref());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if buttons::button(ui, buttons::Icon::Export, "Export").clicked() {
                    self.player = None;
                    editor.export_open = true;
                }
                if buttons::enabled(
                    ui,
                    buttons::Icon::SaveProject,
                    "Save project",
                    editor.project.is_some(),
                )
                .clicked()
                {
                    self.player = None;
                    match platform::save_project(std::path::Path::new(&editor.project_path)) {
                        Ok(Some(path)) => {
                            editor.project_path = path.to_string_lossy().into_owned();
                            editor.save_project();
                        }
                        Ok(None) => {}
                        Err(e) => editor.error = e.to_string(),
                    }
                }
                if buttons::button(ui, buttons::Icon::OpenProject, "Open project").clicked() {
                    self.player = None;
                    match platform::choose_project() {
                        Ok(Some(path)) => {
                            let destination = navigation::Destination::Project(path);
                            if !editor.defer_leave(destination.clone()) {
                                editor.open_project(destination);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => editor.error = e.to_string(),
                    }
                }
                if buttons::enabled(ui, buttons::Icon::Redo, "Redo", !editor.redo.is_empty())
                    .on_hover_text("Ctrl+Shift+Z")
                    .clicked()
                {
                    editor.redo();
                    self.player = None;
                }
                if buttons::enabled(ui, buttons::Icon::Undo, "Undo", !editor.undo.is_empty())
                    .on_hover_text("Ctrl+Z")
                    .clicked()
                {
                    editor.undo();
                    self.player = None;
                }
            });
        });
        if editor.leave.is_none() && !ui.ctx().egui_wants_keyboard_input() {
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
        editor.export_dialog(ui, &self.media);
        editor.job_status(ui);
        self.editor = Some(editor);
    }
}

impl Gavnel {
    pub(super) fn open_edit_project(&mut self) {
        match platform::choose_project() {
            Ok(Some(path)) => match EditProject::load(&path) {
                Ok(project) => {
                    self.player = None;
                    let mut editor = Editor::new(self.settings.clips_dir.clone());
                    editor.id = self.started.elapsed().as_nanos() as u64;
                    editor.frame_id = editor.id;
                    editor.saved_project = Some(project.clone());
                    editor.project = Some(project);
                    editor.loading = false;
                    editor.project_path = path.to_string_lossy().into_owned();
                    self.editor = Some(editor);
                    self.page = Page::Editor;
                }
                Err(e) => self.message = e.to_string(),
            },
            Ok(None) => {}
            Err(e) => self.message = e.to_string(),
        }
    }
}
