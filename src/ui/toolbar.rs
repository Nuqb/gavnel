use super::views::{Glyph, icon_button, paint_icon};
use super::*;
use egui::{Align, Align2, FontId, Layout, Rect, Sense};

impl Gavnel {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, status: &Status) {
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
                opened_from_toolbar = true;
            }
            ui.add_space(2.0);
            let clips = super::clips_menu::clip_button(
                ui,
                &self.settings.hotkey,
                self.settings.replay_seconds,
                self.clips_open,
            );
            let clips_clicked = clips.clicked();
            if clips_clicked {
                self.clips_open = !self.clips_open;
                self.quick_open = false;
            }
            if ui
                .add_enabled(
                    self.settings.recording_mode != RecordingMode::Off,
                    egui::Button::new(format!(
                        "{}  ·  {}",
                        if self.settings.long_recording_hotkey.is_empty() {
                            tr("Not set").to_owned()
                        } else {
                            crate::key_labels::display(&self.settings.long_recording_hotkey)
                        },
                        if status.full_recording {
                            tr("Stop & save")
                        } else {
                            tr("Long recording")
                        }
                    ))
                    .min_size(Vec2::new(165.0, 38.0)),
                )
                .on_hover_text(tr("Start or stop a long recording"))
                .clicked()
            {
                self.toggle_long_recording();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(
                        status.recording && status.buffered > 0.0,
                        egui::Button::new(
                            RichText::new(if status.full_recording {
                                tr("Add bookmark")
                            } else {
                                tr("Save clip")
                            })
                            .strong()
                            .color(Color32::from_rgb(28, 17, 9)),
                        )
                        .fill(ACCENT)
                        .corner_radius(6.0)
                        .min_size(Vec2::new(92.0, 36.0)),
                    )
                    .clicked()
                {
                    self.send(Command::Save);
                }
                if ui
                    .add(
                        egui::Button::new(self.recording_button(status))
                            .corner_radius(6.0)
                            .min_size(Vec2::new(95.0, 36.0)),
                    )
                    .clicked()
                {
                    self.toggle_recording(status);
                }
                if icon_button(
                    ui,
                    Glyph::Mic,
                    if self.settings.microphone {
                        ACCENT
                    } else {
                        MUTED
                    },
                    tr("Quick audio settings"),
                )
                .clicked()
                {
                    self.clips_open = false;
                    self.quick_page = QuickPage::Audio;
                    self.quick_open = true;
                    opened_from_toolbar = true;
                }
                if icon_button(ui, Glyph::Folder, MUTED, tr("Choose clip folder")).clicked() {
                    self.browse_clip_folder();
                }
            });
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
            self.clips_menu(ui, &clips, clips_clicked);
        });
    }
}
