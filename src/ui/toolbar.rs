use super::views::{Glyph, paint_icon};
use super::*;
use egui::{Align2, FontId, Rect, Sense};

impl Gavnel {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.ctx()
            .data_mut(|data| data.remove::<egui::Rect>(egui::Id::unique("video-popup-rect")));
        let popup_id = ui.make_persistent_id("quick-settings-panel");
        ui.horizontal_centered(|ui| {
            let mut opened_from_toolbar = false;
            // Leave room for longer translated controls in the minimum-size window.
            let status_width = (ui.available_width() - 800.0).clamp(140.0, 205.0);
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(status_width, 38.0), Sense::click());
            ui.painter().rect_filled(
                rect,
                8.0,
                if response.hovered() {
                    Color32::from_rgb(45, 45, 50)
                } else {
                    CARD
                },
            );
            paint_icon(
                ui.painter(),
                Rect::from_center_size(
                    egui::pos2(rect.left() + 21.0, rect.center().y),
                    Vec2::splat(18.0),
                ),
                Glyph::Game,
                if status.recording { ACCENT } else { MUTED },
            );
            let label = if status.recording {
                if status.full_recording {
                    format!("REC  {}", recording_time(status.elapsed))
                } else {
                    status.source.chars().take(20).collect()
                }
            } else if status.armed {
                tr("Waiting for game").to_owned()
            } else {
                tr("Recorder off").to_owned()
            };
            let text_rect = Rect::from_min_max(
                egui::pos2(rect.left() + 39.0, rect.top()),
                egui::pos2(rect.right() - 24.0, rect.bottom()),
            );
            ui.painter().with_clip_rect(text_rect).text(
                text_rect.left_center(),
                Align2::LEFT_CENTER,
                &label,
                FontId::proportional(13.0),
                TEXT,
            );
            super::motion::chevron(
                ui,
                popup_id,
                Rect::from_center_size(
                    egui::pos2(rect.right() - 14.0, rect.center().y),
                    Vec2::splat(12.0),
                ),
                self.quick_open,
                MUTED,
            );
            let response = response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("{label}\n{}", tr("Recording settings")));
            if response.clicked() {
                self.quick_page = QuickPage::Overview;
                self.quick_open = !self.quick_open;
                self.clips_open = false;
                self.long_recording_menu_open = false;
                opened_from_toolbar = true;
            }
            ui.add_space(2.0);
            let clips = super::clips_menu::clip_button(
                ui,
                &self.settings.hotkey,
                self.settings.replay_seconds,
                self.clips_open,
            );
            let long_recording = super::clips_menu::shortcut_button(
                ui,
                &self.settings.long_recording_hotkey,
                tr("Long recording"),
                self.long_recording_menu_open,
            );
            let clips_clicked = clips.clicked();
            let long_clicked = long_recording.clicked();
            if clips_clicked || long_clicked {
                if self.clips_capture_owned {
                    self.capturing_hotkey.store(false, Ordering::Relaxed);
                    self.clips_capture_owned = false;
                }
                if clips_clicked {
                    self.clips_open = !self.clips_open;
                    self.long_recording_menu_open = false;
                } else {
                    self.long_recording_menu_open = !self.long_recording_menu_open;
                    self.clips_open = false;
                }
                self.quick_open = false;
            }
            let mut open = self.quick_open;
            super::widgets::quick_panel(
                ui,
                &response,
                popup_id,
                &mut open,
                opened_from_toolbar,
                |ui| match self.quick_page {
                    QuickPage::Overview => self.quick_overview(ui, status),
                    QuickPage::Quality => self.quick_quality(ui),
                    QuickPage::Audio => self.quick_audio(ui),
                },
            );
            self.quick_open = open && self.quick_open && self.source_picker.is_none();
            if self.long_recording_menu_open {
                self.clips_menu(ui, &long_recording, long_clicked, true);
            } else {
                self.clips_menu(ui, &clips, clips_clicked, false);
            }
        });
    }
}
