use super::views::*;
use super::*;
use egui::{Align, Layout, Sense};
impl Gavnel {
    fn source_controls(&mut self, ui: &mut egui::Ui) {
        if self.settings.recording_mode.automatic() {
            ui.label(RichText::new(tr("Entire sessions use automatic game detection. Choose Manual capture to record a monitor or window with the long-recording shortcut.")).color(MUTED));
            if ui.button(tr("Manage supported games")).clicked() {
                self.page = Page::Settings;
                self.settings_page = SettingsPage::Games;
            }
            return;
        }
        let width = ui.available_width().min(480.0);
        ui.allocate_ui_with_layout(Vec2::new(width,0.0),Layout::top_down(Align::Min),|ui|{
            super::widgets::segments(ui,&mut self.settings.mode,&[(CaptureMode::Automatic,tr("Game")),(CaptureMode::Monitor,tr("Monitor")),(CaptureMode::Window,tr("Window"))]);
            ui.add_space(10.0);
            match self.settings.mode{
                CaptureMode::Automatic=>{ui.label(RichText::new(tr("Automatically detect thousands of supported games and your custom game profiles.")).size(13.0).color(MUTED));},
                CaptureMode::Monitor=>{super::motion::ComboBox::from_id_salt("capture-monitor").width(width).selected_text(self.monitors.iter().find(|m|m.index==self.settings.monitor_index).map(|m|m.label.as_str()).unwrap_or(tr("Choose a monitor"))).show_ui(ui,|ui|{for m in &self.monitors{ui.selectable_value(&mut self.settings.monitor_index,m.index,&m.label);}});ui.label(RichText::new(tr("Includes everything visible on this monitor after Alt-Tab.")).size(12.0).color(MUTED));},
                CaptureMode::Window=>{super::motion::ComboBox::from_id_salt("capture-window").width(width).selected_text(if self.settings.window_title.is_empty(){tr("Choose a running program")}else{&self.settings.window_title}).show_ui(ui,|ui|{for w in &self.windows{if ui.selectable_label(self.settings.window_title==w.title,&w.title).clicked(){self.settings.window_title=w.title.clone();self.settings.window_executable=w.executable.clone();}}});ui.label(RichText::new(tr("Video goes black when unfocused; audio continues.")).size(12.0).color(MUTED));}
            }
            if ui.add(egui::Button::new(tr("Refresh sources")).min_size(Vec2::new(130.0,34.0))).clicked(){self.windows=sources::windows();self.monitors=sources::monitors();}
        });
    }
    pub(super) fn capture(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.label(RichText::new(tr("Capture")).size(27.0).strong());
        ui.label(
            RichText::new(tr(
                "Record a full session or keep recent moments ready to clip.",
            ))
            .color(MUTED),
        );
        ui.add_space(20.0);
        section(ui, tr("Recorder"), |ui| {
            ui.horizontal(|ui| {
                badge(
                    ui,
                    if status.recording {
                        "RECORDING"
                    } else if status.armed {
                        tr("WAITING FOR GAME")
                    } else {
                        tr("OFF")
                    },
                    if status.armed { ACCENT_DARK } else { CARD },
                    if status.armed { ACCENT } else { MUTED },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}p · {} FPS · {} Mbps",
                            self.settings.height, self.settings.fps, self.settings.bitrate_mbps
                        ))
                        .color(MUTED),
                    );
                });
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(if status.source.is_empty() {
                    tr("No game selected")
                } else {
                    &status.source
                })
                .size(20.0)
                .strong(),
            );
            if !status.backend.is_empty() {
                ui.label(RichText::new(&status.backend).color(MUTED).size(12.0));
            }
            ui.add_space(13.0);
            if status.full_recording && status.recording {
                ui.label(
                    RichText::new(format!("REC  {}", super::recording_time(status.elapsed)))
                        .size(24.0)
                        .strong()
                        .color(ACCENT),
                );
                ui.label(
                    RichText::new(tr("Writing full recording to disk · Stop & save to finish"))
                        .size(12.0)
                        .color(MUTED),
                );
                ui.add_space(8.0);
            }
            ui.add(
                egui::ProgressBar::new(
                    (status.buffered / self.settings.replay_seconds as f64).min(1.0) as f32,
                )
                .fill(ACCENT)
                .desired_height(6.0),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(i18n::message(
                        "{buffered} / {seconds} s buffered",
                        &[
                            ("buffered", &format!("{:.0}", status.buffered)),
                            ("seconds", &self.settings.replay_seconds.to_string()),
                        ],
                    ))
                    .color(MUTED)
                    .size(12.0),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(i18n::message(
                            "{size} replay memory",
                            &[("size", &crate::storage::format_bytes(status.memory as u64))],
                        ))
                        .color(MUTED)
                        .size(12.0),
                    );
                });
            });
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if orange_button(ui, self.recording_button(status)).clicked() {
                    self.toggle_recording(status);
                }
                if ui
                    .add_enabled(
                        status.recording && status.buffered > 0.0,
                        egui::Button::new(format!(
                            "{}  ·  {}",
                            if status.full_recording {
                                tr("Add bookmark")
                            } else {
                                tr("Save clip")
                            },
                            crate::key_labels::display(&self.settings.hotkey)
                        )),
                    )
                    .clicked()
                {
                    self.send(Command::Save);
                }
            });
        });
        ui.add_space(14.0);
        section(ui, tr("Video capture source"), |ui| {
            self.source_controls(ui);
            ui.add_space(10.0);
            if ui.button(tr("Apply source")).clicked()
                && let Err(e) = self.apply()
            {
                self.message = e.to_string();
            }
        });
        ui.add_space(14.0);
        section(ui, tr("Audio"), |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                paint_icon(ui.painter(), r, Glyph::Volume, MUTED);
                ui.label(tr("Automatic PC audio"));
                ui.separator();
                ui.label(if self.settings.microphone {
                    "Microphone included"
                } else {
                    tr("Microphone off")
                });
                if ui.button(tr("Configure")).clicked() {
                    self.page = Page::Settings;
                    self.settings_page = SettingsPage::Audio;
                }
            });
        });
        ui.add_space(15.0);
        ui.label(
            RichText::new(i18n::message("Delivered: {delivered} FPS · Fresh source: {fresh} FPS · Dropped: {dropped} · Captures saved: {saved}", &[("delivered", &format!("{:.1}", status.capture_fps)), ("fresh", &format!("{:.1}", status.fresh_fps)), ("dropped", &status.dropped.to_string()), ("saved", &status.saved_count.to_string())]))
            .color(MUTED)
            .size(12.0),
        );
    }
}
