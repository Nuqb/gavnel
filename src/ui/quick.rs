use super::views::{Glyph, paint_icon};
use super::widgets::{arrow, toggle};
use super::*;
use egui::{Align, Align2, FontId, Layout, Rect, Sense, Stroke};

impl Gavnel {
    pub(super) fn quick_overview(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("RECORD WITH GAVNEL")
                    .size(10.5)
                    .strong()
                    .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut enabled = status.armed;
                if toggle(ui, &mut enabled).changed() {
                    self.toggle_recording(status);
                }
                ui.label(
                    RichText::new(if status.armed { "ON" } else { "OFF" })
                        .size(10.0)
                        .color(MUTED),
                );
            });
        });
        if status.recording {
            self.engine.preview_requested.store(true, Ordering::Relaxed);
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
        let response = super::source_card::show(ui, status);
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.open_source_picker();
        }
        ui.add_space(10.0);
        let mode = match self.settings.recording_mode {
            RecordingMode::Clips => "Manual capture",
            RecordingMode::FullRecording => "Manual capture",
            RecordingMode::AutomaticSessions => "Entire game sessions",
            RecordingMode::Off => "Recording off",
        };
        if quick_row(ui, Glyph::Record, "Recording Mode", mode).clicked() {
            self.page = Page::Settings;
            self.settings_page = SettingsPage::Recording;
            self.quick_open = false;
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new("focus-recording-mode"), true));
            ui.close();
        }
        ui.add_space(5.0);
        ui.separator();
        let mode = match self.settings.mode {
            CaptureMode::Automatic => "Game",
            CaptureMode::Monitor => "Monitor",
            CaptureMode::Window => "Window",
        };
        if quick_row(ui, Glyph::Game, "Video capture source", mode).clicked() {
            self.open_source_picker();
        }
        if quick_row(
            ui,
            Glyph::Settings,
            "Quality",
            &format!("{}p {} FPS", self.settings.height, self.settings.fps),
        )
        .clicked()
        {
            self.quick_page = QuickPage::Quality;
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
            paint_icon(ui.painter(), r, Glyph::Mic, TEXT);
            ui.label(RichText::new("Microphone").size(13.0));
            let mut changed = false;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if !self.settings.microphone {
                    "Off"
                } else {
                    self.inputs
                        .iter()
                        .find(|d| d.id == self.settings.input_device)
                        .map(|d| d.label.as_str())
                        .unwrap_or("Windows default")
                };
                super::motion::ComboBox::from_id_salt("quick-mic")
                    .width(150.0)
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(!self.settings.microphone, "Off")
                            .clicked()
                        {
                            self.settings.microphone = false;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                self.settings.microphone && self.settings.input_device.is_empty(),
                                "Windows default",
                            )
                            .clicked()
                        {
                            self.settings.microphone = true;
                            self.settings.input_device.clear();
                            changed = true;
                        }
                        for device in &self.inputs {
                            if ui
                                .selectable_label(
                                    self.settings.microphone
                                        && self.settings.input_device == device.id,
                                    &device.label,
                                )
                                .clicked()
                            {
                                self.settings.microphone = true;
                                self.settings.input_device = device.id.clone();
                                changed = true;
                            }
                        }
                    });
            });
            if changed && let Err(e) = self.apply() {
                self.message = e.to_string();
            }
        });
        if quick_row(ui, Glyph::Volume, "Audio sources", "Automatic PC audio").clicked() {
            self.quick_page = QuickPage::Audio;
        }
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(3.0);
        let folder = self
            .settings
            .clips_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(15)
            .collect::<String>();
        if quick_row(ui, Glyph::Folder, "Clip folder", &folder)
            .on_hover_text(self.settings.clips_dir.display().to_string())
            .clicked()
        {
            self.browse_clip_folder();
        }
        if quick_row(ui, Glyph::Settings, "Recorder settings", "").clicked() {
            self.page = Page::Settings;
            self.settings_page = SettingsPage::Recording;
            ui.close();
        }
    }
    pub(super) fn quick_quality(&mut self, ui: &mut egui::Ui) {
        self.quick_header(ui, "Recording quality");
        let previous = (
            self.settings.height,
            self.settings.fps,
            self.settings.replay_seconds,
            self.settings.bitrate_mbps,
        );
        ui.label(RichText::new("Resolution").size(12.0).color(MUTED));
        super::widgets::dropdown(
            ui,
            "quick-resolution",
            &mut self.settings.height,
            &[(720, "720p"), (1080, "1080p"), (1440, "1440p")],
        );
        ui.label(RichText::new("Frame rate").size(12.0).color(MUTED));
        super::widgets::dropdown(
            ui,
            "quick-fps",
            &mut self.settings.fps,
            &[(30, "30 FPS"), (60, "60 FPS"), (120, "120 FPS")],
        );
        ui.label(RichText::new("Clip length").size(12.0).color(MUTED));
        ui.horizontal(|ui| {
            super::motion::ComboBox::from_id_salt("quick-duration")
                .width(155.0)
                .selected_text(format!("{} seconds", self.settings.replay_seconds))
                .show_ui(ui, |ui| {
                    for seconds in [15, 30, 60, 120] {
                        ui.selectable_value(
                            &mut self.settings.replay_seconds,
                            seconds,
                            format!("{seconds} seconds"),
                        );
                    }
                });
        });
        ui.label(RichText::new("Bitrate").size(12.0).color(MUTED));
        let bitrate = super::widgets::slider(ui, &mut self.settings.bitrate_mbps, 5..=50, "Mbps");
        let options_changed = previous.0 != self.settings.height
            || previous.1 != self.settings.fps
            || previous.2 != self.settings.replay_seconds;
        if (options_changed || bitrate.drag_stopped() || (bitrate.changed() && !bitrate.dragged()))
            && let Err(e) = self.apply()
        {
            self.message = e.to_string();
        }
        ui.add_space(7.0);
        ui.label(
            RichText::new("Quality changes save the current full recording and begin a new file, or reset the replay buffer.")
                .size(11.0)
                .color(MUTED),
        );
    }
    pub(super) fn quick_audio(&mut self, ui: &mut egui::Ui) {
        self.quick_header(ui, "Audio sources");
        let mut changed = false;
        ui.label(RichText::new("PC audio · Always recorded").strong());
        ui.label(
            RichText::new("Game, Discord, and all active playback devices.")
                .size(11.0)
                .color(MUTED),
        );
        let level = super::widgets::slider(ui, &mut self.settings.desktop_volume, 0.0..=2.0, "%");
        changed |= level.drag_stopped() || (level.changed() && !level.dragged());
        ui.add_space(3.0);
        ui.separator();
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label("Include microphone");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                changed |= toggle(ui, &mut self.settings.microphone).changed();
            });
        });
        let before = self.settings.input_device.clone();
        super::views::device_selector(
            ui,
            "quick-input-device",
            &mut self.settings.input_device,
            &self.inputs,
        );
        changed |= before != self.settings.input_device;
        let level = super::widgets::slider(ui, &mut self.settings.mic_volume, 0.0..=2.0, "%");
        changed |= level.drag_stopped() || (level.changed() && !level.dragged());
        changed |= super::views::mono_controls(ui, &mut self.settings);
        if changed && let Err(e) = self.apply() {
            self.message = e.to_string();
        }
        if ui
            .add(egui::Button::new("Refresh audio devices").frame_when_inactive(false))
            .clicked()
        {
            self.inputs = audio::devices(true);
        }
    }
    fn quick_header(&mut self, ui: &mut egui::Ui, title: &str) {
        ui.horizontal(|ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(25.0, 28.0), Sense::click());
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, 5.0, Color32::from_rgb(48, 48, 54));
            }
            let center = rect.center();
            ui.painter().add(egui::Shape::line(
                vec![
                    center + Vec2::new(3.0, -5.0),
                    center + Vec2::new(-2.0, 0.0),
                    center + Vec2::new(3.0, 5.0),
                ],
                Stroke::new(1.5_f32, TEXT),
            ));
            let response = response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Back to recording settings");
            if response.clicked() {
                self.quick_page = QuickPage::Overview;
            }
            ui.label(RichText::new(title).size(15.0).strong());
        });
        ui.add_space(8.0);
    }
}
fn quick_row(ui: &mut egui::Ui, glyph: Glyph, label: &str, value: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(332.0, 39.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 6.0, Color32::from_rgb(43, 43, 47));
    }
    paint_icon(
        ui.painter(),
        Rect::from_center_size(
            egui::pos2(rect.left() + 12.0, rect.center().y),
            Vec2::splat(18.0),
        ),
        glyph,
        TEXT,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 30.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(13.0, egui::FontFamily::Name("GavnelSemibold".into())),
        TEXT,
    );
    ui.painter()
        .with_clip_rect(Rect::from_min_max(
            egui::pos2(rect.right() - 134.0, rect.top()),
            egui::pos2(rect.right() - 24.0, rect.bottom()),
        ))
        .text(
            egui::pos2(rect.right() - 26.0, rect.center().y),
            Align2::RIGHT_CENTER,
            value,
            FontId::proportional(12.0),
            MUTED,
        );
    arrow(
        ui.painter(),
        Rect::from_center_size(
            egui::pos2(rect.right() - 9.0, rect.center().y),
            Vec2::splat(12.0),
        ),
        false,
        MUTED,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
