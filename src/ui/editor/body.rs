use super::*;
impl Editor {
    pub(super) fn body(
        &mut self,
        ui: &mut egui::Ui,
        service: &MediaService,
        player: &mut Option<crate::player::Player>,
    ) {
        self.take_play_shortcut(ui);
        if let Some(before) = self.project.clone() {
            self.selected = self.selected.min(before.segments.len().saturating_sub(1));
            let old_selection = self.selected;
            if before.segments.len() > 1 {
                self.clip_strip(ui);
            }
            if old_selection != self.selected {
                if let Some((_, cancel)) = self.preview_job.take() {
                    cancel.store(true, Ordering::Release);
                }
                *player = None;
                self.texture = None;
                self.last_frame = None;
            }
            ui.add_space(8.0);
            let previous_tab = self.tab;
            let available = ui.available_size();
            let preview_h = (available.y - 220.0).clamp(140.0, 430.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new((available.x - 280.0).max(320.0), preview_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.preview_canvas(ui, player);
                    },
                );
                ui.allocate_ui_with_layout(
                    Vec2::new(260.0, preview_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(260.0);
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            self.controls(ui);
                        });
                    },
                );
            });
            if previous_tab != Tab::Crop && self.tab == Tab::Crop {
                if let Some(p) = player.as_ref() {
                    let _ = p.pause();
                }
                self.last_frame = None;
            }
            let before_cursor = self.cursor;
            self.timeline(ui);
            if (self.cursor - before_cursor).abs() > 0.0001 {
                if let Some((_, cancel)) = self.preview_job.take() {
                    cancel.store(true, Ordering::Release);
                }
                *player = None;
            }
            ui.horizontal(|ui| {
                let playing = player.as_ref().is_some_and(|p| !p.paused() && !p.ended());
                if buttons::button(
                    ui,
                    if playing {
                        buttons::Icon::Pause
                    } else {
                        buttons::Icon::Play
                    },
                    if playing { "Pause" } else { "Play" },
                )
                .on_hover_text(tr("Play / pause · Space"))
                .clicked()
                {
                    self.play_requested = true;
                }
                if self.preview_job.is_some() {
                    ui.spinner();
                    ui.label(tr("Preparing playback…"));
                    if ui.button(tr("Cancel")).clicked()
                        && let Some((_, cancel)) = self.preview_job.take()
                    {
                        cancel.store(true, Ordering::Release);
                    }
                }
                ui.label(format!(
                    "{} / {}",
                    playback::time(self.cursor),
                    playback::time(
                        self.project.as_ref().unwrap().segments[self.selected]
                            .info
                            .duration
                    )
                ));
            });
            let changed = self.project.as_ref() != Some(&before);
            if changed {
                *player = None;
                if let Some((_, cancel)) = self.preview_job.take() {
                    cancel.store(true, Ordering::Release);
                }
            }
            self.record_edit(before, ui.input(|i| i.pointer.any_down()));
            if player.as_ref().is_none_or(|p| p.paused()) && !ui.input(|i| i.pointer.any_down()) {
                self.request_frame(service);
                self.request_filmstrip(service);
            }
            if self.frame_pending || self.strip_pending.is_some() {
                ui.ctx().request_repaint_after(Duration::from_millis(50));
            }
        }
    }
}
