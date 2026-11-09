use super::views::*;
use super::*;
use egui::{Align, Layout, Pos2, Rect, Sense};
impl Gavnel {
    pub(super) fn settings_audio(&mut self, ui: &mut egui::Ui) {
        section(ui, "Recording Audio", |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Audio Processes").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    super::motion::ComboBox::from_id_salt("audio-processes")
                        .selected_text(if self.settings.output_devices.is_none() {
                            "All PC audio"
                        } else {
                            "Selected playback devices"
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.settings.output_devices.is_none(),
                                    "All PC audio",
                                )
                                .clicked()
                            {
                                self.settings.output_devices = None;
                            }
                            if ui
                                .selectable_label(
                                    self.settings.output_devices.is_some(),
                                    "Selected playback devices",
                                )
                                .clicked()
                            {
                                self.settings.output_devices.get_or_insert_with(|| {
                                    self.outputs.iter().map(|d| d.id.clone()).collect()
                                });
                            }
                        });
                });
            });
            ui.add_space(12.0);
            egui::Frame::new()
                .fill(CARD)
                .corner_radius(6.0)
                .inner_margin(14.0)
                .show(ui, |ui| {
                    audio_volume_row(
                        ui,
                        "Microphone",
                        Glyph::Mic,
                        &mut self.settings.microphone,
                        &mut self.settings.mic_volume,
                    );
                    ui.separator();
                    audio_volume_row(
                        ui,
                        "PC Audio",
                        Glyph::Volume,
                        &mut self.settings.desktop_audio,
                        &mut self.settings.desktop_volume,
                    );
                    ui.add_enabled_ui(self.settings.desktop_audio, |ui| {
                        ui.indent("output-devices", |ui| {
                            let mut all = self.settings.output_devices.is_none();
                            if ui.checkbox(&mut all, "All playback devices").changed() {
                                self.settings.output_devices = if all {
                                    None
                                } else {
                                    Some(self.outputs.iter().map(|d| d.id.clone()).collect())
                                };
                            }
                            for device in &self.outputs {
                                let mut selected = self
                                    .settings
                                    .output_devices
                                    .as_ref()
                                    .is_none_or(|ids| ids.contains(&device.id));
                                if ui.checkbox(&mut selected, &device.label).changed() {
                                    let ids =
                                        self.settings.output_devices.get_or_insert_with(|| {
                                            self.outputs.iter().map(|d| d.id.clone()).collect()
                                        });
                                    ids.retain(|id| id != &device.id);
                                    if selected {
                                        ids.push(device.id.clone());
                                    }
                                }
                            }
                        });
                    });
                });
            ui.separator();
            audio_toggle(
                ui,
                "Separate Audio Tracks",
                &mut self.settings.separate_audio_tracks,
            );
        });
        ui.add_space(14.0);
        if self
            .mic_test
            .as_ref()
            .is_some_and(|test| !test.matches_settings(&self.settings))
        {
            self.mic_test = None;
            self.message =
                "Microphone settings changed. Start a fresh Mic Test to hear them.".into();
        }
        section(ui, "Microphone Settings", |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Microphone").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    device_selector(
                        ui,
                        "input-device",
                        &mut self.settings.input_device,
                        &self.inputs,
                    );
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Mic Test").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(test) = &self.mic_test {
                        match test.state() {
                            audio::MicTestState::Recording => {
                                if ui.button("Playback").clicked() {
                                    test.playback();
                                    ui.ctx().request_repaint_after(Duration::from_millis(30));
                                }
                                if ui.button("End Test").clicked() {
                                    test.end();
                                }
                            }
                            audio::MicTestState::Playing => {
                                if ui.button("Stop Playback").clicked() {
                                    test.stop_playback();
                                }
                            }
                            audio::MicTestState::Ready => {
                                if ui.button("Start Test").clicked() {
                                    self.mic_test =
                                        Some(audio::MicTest::start(self.settings.clone()));
                                } else if test.has_recording() && ui.button("Playback").clicked() {
                                    test.playback();
                                    ui.ctx().request_repaint_after(Duration::from_millis(30));
                                }
                            }
                        }
                    } else if ui.button("Start Test").clicked() {
                        self.mic_test = Some(audio::MicTest::start(self.settings.clone()));
                    }
                });
            });
            let level = self.mic_test.as_ref().map_or(0.0, |test| test.level());
            if let Some(test) = &self.mic_test {
                if let Some(error) = test.error() {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                if test.state() != audio::MicTestState::Ready {
                    ui.ctx().request_repaint_after(Duration::from_millis(30));
                }
            }
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
            for i in 0..70 {
                let x = rect.left() + i as f32 * rect.width() / 70.0;
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        Pos2::new(x, rect.top()),
                        Vec2::new((rect.width() / 70.0 - 3.0).max(1.0), rect.height()),
                    ),
                    0.0,
                    if (i as f32) < level * 70.0 {
                        ACCENT
                    } else {
                        MUTED.gamma_multiply(0.45)
                    },
                );
            }
            if self.settings.input_sensitivity_enabled && !self.settings.push_to_talk {
                let x = rect.left()
                    + rect.width() * ((self.settings.input_sensitivity_db + 100.0) / 100.0);
                ui.painter().line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    egui::Stroke::new(2.0_f32, TEXT),
                );
            }
            ui.label(
                RichText::new("Mic level: -100 to 0 dB · White line: sensitivity threshold")
                    .size(11.0)
                    .color(MUTED),
            );
            ui.separator();
            audio_toggle(
                ui,
                "Input Sensitivity",
                &mut self.settings.input_sensitivity_enabled,
            );
            ui.label(RichText::new("Mutes the mic below your threshold. Lower values pick up quieter speech. Push to Talk bypasses this threshold.").size(12.0).color(MUTED));
            ui.add_enabled_ui(
                self.settings.input_sensitivity_enabled && !self.settings.push_to_talk,
                |ui| {
                    super::widgets::slider(
                        ui,
                        &mut self.settings.input_sensitivity_db,
                        -100.0..=-25.0,
                        "dB",
                    );
                },
            );
            ui.separator();
            ui.label(RichText::new("Noise Suppression").strong());
            use crate::config::NoiseSuppression;
            super::motion::ComboBox::from_id_salt("mic-noise-suppression")
                .selected_text(self.settings.noise_suppression_mode.label())
                .show_ui(ui, |ui| {
                    for mode in [
                        NoiseSuppression::None,
                        NoiseSuppression::Standard,
                        NoiseSuppression::Neural,
                    ] {
                        ui.selectable_value(
                            &mut self.settings.noise_suppression_mode,
                            mode,
                            mode.label(),
                        );
                    }
                });
            ui.label(RichText::new(match self.settings.noise_suppression_mode {
                            NoiseSuppression::None => "Keeps the original mic sound. Best for a quiet room or an already filtered mic.",
                            NoiseSuppression::Standard => "Reduces steady fan noise and hiss. Use AI for typing and other changing background noise.",
                            NoiseSuppression::Neural => "On-device AI filtering for background noise such as typing and fans. May change singing or music.",
                        }).size(12.0).color(MUTED));
            ui.label(RichText::new("Changing microphone settings clears the old test. Start Test to hear the current settings.").size(12.0).color(MUTED));
            ui.separator();
            audio_toggle(ui, "Push To Talk", &mut self.settings.push_to_talk);
            ui.add_enabled_ui(self.settings.push_to_talk, |ui| {
                let label = if self.capturing_ptt {
                    "Press a key…".to_owned()
                } else {
                    format!(
                        "Hotkey: {}",
                        audio::ptt_key_name(self.settings.push_to_talk_key)
                    )
                };
                if ui.button(label).clicked() {
                    self.capturing_ptt = true;
                }
                if self.capturing_ptt {
                    for key in 8..=254u16 {
                        if unsafe {
                            windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
                                key as i32,
                            )
                        } < 0
                        {
                            if key != 27 {
                                self.settings.push_to_talk_key = key;
                            }
                            self.capturing_ptt = false;
                            break;
                        }
                    }
                    ui.ctx().request_repaint_after(Duration::from_millis(30));
                }
            });
            ui.separator();
            mono_controls(ui, &mut self.settings);
        });
        ui.add_space(14.0);
        if ui.button("Refresh audio devices").clicked() {
            self.inputs = audio::devices(true);
            self.outputs = audio::devices(false);
        }
    }
}
