use super::*;
pub(super) struct Host<'a> {
    pub parent: windows::Win32::Foundation::HWND,
    pub context: &'a egui::Context,
    pub visible: &'a Arc<AtomicBool>,
}
impl Editor {
    pub(super) fn take_play_shortcut(&mut self, ui: &mut egui::Ui) {
        if self.leave.is_some() || self.save_open || ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let pressed = ui.input_mut(|i| {
            let fresh = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Space,
                        pressed: true,
                        repeat: false,
                        ..
                    }
                )
            });
            fresh && i.consume_key(egui::Modifiers::NONE, egui::Key::Space)
        });
        self.play_requested |= pressed;
    }
    pub(super) fn toggle_playback(
        &mut self,
        service: &MediaService,
        player: &mut Option<crate::player::Player>,
        host: Host<'_>,
    ) {
        if self.save_open {
            return;
        }
        if let Some((_, cancel)) = self.preview_job.take() {
            cancel.store(true, Ordering::Release);
            return;
        }
        let Some(segment) = self
            .project
            .as_ref()
            .and_then(|p| p.segments.get(self.selected))
        else {
            return;
        };
        if let Some(p) = player {
            let restart = p.ended() || self.cursor >= segment.end - 0.5 / segment.info.fps;
            if p.ended() || self.cursor >= segment.end - 0.5 / segment.info.fps {
                let start = if self.rendered_playback {
                    0.0
                } else {
                    segment.start
                };
                let _ = p.seek(start);
                self.cursor = if self.rendered_playback {
                    self.preview_origin
                } else {
                    segment.start
                };
            }
            let result = if restart { p.play() } else { p.toggle() };
            if let Err(e) = result {
                self.error = e.to_string();
            }
            return;
        }
        if self.cursor < segment.start || self.cursor >= segment.end - 0.5 / segment.info.fps {
            self.cursor = segment.start;
        }
        if segment
            .audio
            .iter()
            .any(|a| a.gain != 1.0 || a.muted || a.solo)
        {
            self.start_preview(service);
            return;
        }
        match crate::player::Player::new(
            segment.info.path.clone(),
            host.parent,
            host.context.clone(),
            host.visible.clone(),
        ) {
            Ok(p) => {
                if let Err(e) = p.seek(self.cursor) {
                    self.error = e.to_string();
                }
                self.preview_origin = 0.0;
                self.rendered_playback = false;
                *player = Some(p);
            }
            Err(e) => self.error = e.to_string(),
        }
    }
    pub(super) fn poll_playback(
        &mut self,
        player: &mut Option<crate::player::Player>,
        host: Host<'_>,
    ) {
        let Some((state, _)) = &self.preview_job else {
            return;
        };
        let result = state.lock().unwrap().result.clone();
        let Some(result) = result else {
            return;
        };
        self.preview_job = None;
        match result {
            Ok(path) => {
                match crate::player::Player::new(
                    path,
                    host.parent,
                    host.context.clone(),
                    host.visible.clone(),
                ) {
                    Ok(p) => {
                        self.rendered_playback = true;
                        *player = Some(p);
                    }
                    Err(e) => self.error = e.to_string(),
                }
            }
            Err(e) if e == "Cancelled" => {}
            Err(e) => self.error = e,
        }
    }
    pub(super) fn update_cursor(&mut self, player: &mut Option<crate::player::Player>) {
        let Some(p) = player.as_ref() else {
            return;
        };
        if let Some(error) = p.error() {
            self.error = error;
            *player = None;
            return;
        }
        if p.duration() <= 0.0 {
            return;
        }
        self.cursor = self.preview_origin + p.position();
        if let Some(segment) = self
            .project
            .as_ref()
            .and_then(|p| p.segments.get(self.selected))
            && self.cursor >= segment.end
        {
            let _ = p.pause();
            self.cursor = segment.end;
        }
    }
    fn start_preview(&mut self, service: &MediaService) {
        let Some(project) = &self.project else {
            return;
        };
        let canvas = crate::media::preview_dimensions(project);
        let mut preview = project.clone();
        let mut segment = preview.segments[self.selected].clone();
        segment.start = self
            .cursor
            .clamp(segment.start, (segment.end - 0.1).max(segment.start));
        // Render the remaining selection when track mixing requires a proxy.
        self.preview_origin = segment.start;
        preview.segments = vec![segment];
        let folder = std::env::temp_dir().join("Gavnel/previews");
        let _ = std::fs::create_dir_all(&folder);
        use sha2::{Digest, Sha256};
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&preview, canvas)).unwrap_or_default())
        );
        let path = folder.join(format!("{digest}.mp4"));
        if path.is_file() {
            self.preview_job = Some((
                Arc::new(Mutex::new(JobProgress {
                    fraction: 1.0,
                    message: String::new(),
                    result: Some(Ok(path)),
                })),
                Arc::new(AtomicBool::new(false)),
            ));
            return;
        }
        if let Some((_, cancel)) = &self.preview_job {
            cancel.store(true, Ordering::Release);
        }
        match service.export(
            preview,
            ExportOptions {
                replace_source: false,
                path,
                height: Some(720),
                size_mb: None,
                canvas: Some(canvas),
            },
        ) {
            Ok(job) => self.preview_job = Some(job),
            Err(e) => self.error = e.to_string(),
        }
    }
}
