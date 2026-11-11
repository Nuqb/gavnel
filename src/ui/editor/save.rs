use super::*;

impl Editor {
    pub(super) fn current_video(&self) -> Option<PathBuf> {
        let project = self.project.as_ref()?;
        let path = &project.segments.first()?.info.path;
        project
            .segments
            .iter()
            .all(|s| s.info.path == *path)
            .then(|| path.clone())
    }

    pub(super) fn start_video_save(
        &mut self,
        service: &MediaService,
        path: PathBuf,
        replace_source: bool,
    ) {
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
        {
            self.error = "Choose an MP4 filename.".into();
            return;
        }
        if !replace_source && path.exists() {
            self.error = "A file with that name already exists. Choose another name.".into();
            return;
        }
        let Some(project) = self.project.clone() else {
            return;
        };
        let options = ExportOptions {
            path,
            replace_source,
            height: self.height,
            size_mb: self.size_mb,
            canvas: None,
        };
        match service.export(project.clone(), options.clone()) {
            Ok(job) => {
                self.saving_video = Some(job.0.clone());
                if let Some((_, cancel)) = self.preview_job.take() {
                    cancel.store(true, Ordering::Release);
                }
                self.pending_save = Some(super::super::saving::Job::new(&project, &options, job));
                self.error.clear();
                self.save_open = false;
            }
            Err(error) => self.error = error.to_string(),
        }
    }

    pub(in crate::ui) fn poll_save(&mut self, service: &MediaService) {
        let result = self
            .saving_video
            .as_ref()
            .and_then(|job| job.lock().unwrap().result.clone());
        let Some(result) = result else {
            return;
        };
        self.saving_video = None;
        if let Err(error) = &result {
            self.error = error.clone();
        }
        if let Ok(path) = result {
            if matches!(self.leave, Some(navigation::Destination::Player(_))) {
                self.leave = Some(navigation::Destination::Player(path.clone()));
            }
            if self.return_player.is_some() {
                self.return_player = Some(path.clone());
            }
            // Reload the rendered video so the next save does not apply edits twice.
            self.undo.clear();
            self.redo.clear();
            self.pending = None;
            self.project = None;
            self.saved_project = None;
            self.selected = 0;
            self.cursor = 0.0;
            self.texture = None;
            self.last_frame = None;
            self.filmstrips.clear();
            self.strip_pending = None;
            self.frame_id = self.frame_id.wrapping_add(1);
            self.frame_pending = false;
            self.id = self.id.wrapping_add(1);
            self.loading = true;
            if let Err(error) = service.open(self.id, vec![path]) {
                self.loading = false;
                self.error = error.to_string();
            }
        }
    }
}

impl Editor {
    pub(super) fn save_video_dialog(&mut self, ui: &mut egui::Ui, service: &MediaService) {
        if !self.save_open {
            return;
        }
        match self.save_modal(ui.ctx()) {
            save_dialog::Action::Current => {
                if let Some(path) = self.current_video() {
                    self.start_video_save(service, path, true);
                }
            }
            save_dialog::Action::New => {
                match platform::save_video(&self.folder.join(&self.filename)) {
                    Ok(Some(path)) => self.start_video_save(service, path, false),
                    Ok(None) => {}
                    Err(error) => self.error = error.to_string(),
                }
            }
            save_dialog::Action::Close => self.save_open = false,
            save_dialog::Action::None => {}
        }
    }
}
