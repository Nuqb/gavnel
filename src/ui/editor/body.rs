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
            self.preview_and_controls(ui, player);
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
            self.transport_row(ui, player.as_ref());
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

    fn preview_and_controls(
        &mut self,
        ui: &mut egui::Ui,
        player: &mut Option<crate::player::Player>,
    ) {
        let available = ui.available_size();
        let spacing = ui.spacing();
        let button_height = buttons::ICON_SIZE + 2.0 * spacing.button_padding.y;
        let transport_height = spacing.interact_size.y.max(button_height);
        let footer =
            timeline::HEIGHT + timeline::TOP_GAP + transport_height + 2.0 * spacing.item_spacing.y;
        let preview_height = (available.y - footer).max(140.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new((available.x - 280.0).max(320.0), preview_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.preview_canvas(ui, player),
            );
            ui.allocate_ui_with_layout(
                Vec2::new(260.0, preview_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(260.0);
                    egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui));
                },
            );
        });
    }

    fn transport_row(&mut self, ui: &mut egui::Ui, player: Option<&crate::player::Player>) {
        ui.horizontal(|ui| {
            let playing = player.is_some_and(|p| !p.paused() && !p.ended());
            let (icon, label) = if playing {
                (buttons::Icon::Pause, "Pause")
            } else {
                (buttons::Icon::Play, "Play")
            };
            if buttons::button(ui, icon, label)
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
            let duration = self.project.as_ref().unwrap().segments[self.selected]
                .info
                .duration;
            ui.label(format!(
                "{} / {}",
                playback::time(self.cursor),
                playback::time(duration)
            ));
        });
    }
}
