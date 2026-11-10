use super::views::*;
use super::*;
use egui::{Align, Layout};
impl Gavnel {
    pub(super) fn settings_audio(&mut self, ui: &mut egui::Ui) {
        section(ui, tr("Recording Audio"), |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Audio Processes")).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    super::motion::ComboBox::from_id_salt("audio-processes")
                        .selected_text(if self.settings.output_devices.is_none() {
                            tr("All PC audio")
                        } else {
                            tr("Selected playback devices")
                        })
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.settings.output_devices.is_none(),
                                    tr("All PC audio"),
                                )
                                .clicked()
                            {
                                self.settings.output_devices = None;
                            }
                            if ui
                                .selectable_label(
                                    self.settings.output_devices.is_some(),
                                    tr("Selected playback devices"),
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
                        tr("Microphone"),
                        Glyph::Mic,
                        &mut self.settings.microphone,
                        &mut self.settings.mic_volume,
                    );
                    ui.separator();
                    audio_volume_row(
                        ui,
                        tr("PC Audio"),
                        Glyph::Volume,
                        &mut self.settings.desktop_audio,
                        &mut self.settings.desktop_volume,
                    );
                    ui.add_enabled_ui(self.settings.desktop_audio, |ui| {
                        ui.indent("output-devices", |ui| {
                            let mut all = self.settings.output_devices.is_none();
                            if super::checkbox::checkbox(ui, &mut all, tr("All playback devices"))
                                .changed()
                            {
                                self.settings.output_devices =
                                    if all { None } else { Some(Vec::new()) };
                            }
                            for device in &self.outputs {
                                let mut selected = self
                                    .settings
                                    .output_devices
                                    .as_ref()
                                    .is_none_or(|ids| ids.contains(&device.id));
                                if super::checkbox::checkbox(ui, &mut selected, &device.label)
                                    .changed()
                                {
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
                tr("Separate Audio Tracks"),
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
                tr("Microphone settings changed. Start a fresh Mic Test to hear them.").into();
        }
        section(ui, tr("Microphone Settings"), |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Microphone")).strong());
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
            super::microphone_test::show(ui, &self.settings, &mut self.mic_test);
            ui.separator();
            audio_toggle(
                ui,
                tr("Input Sensitivity"),
                &mut self.settings.input_sensitivity_enabled,
            );
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
            ui.label(RichText::new(tr("Noise Suppression")).strong());
            use crate::config::NoiseSuppression;
            super::motion::ComboBox::from_id_salt("mic-noise-suppression")
                .selected_text(tr(self.settings.noise_suppression_mode.label()))
                .show_ui(ui, |ui| {
                    for mode in [
                        NoiseSuppression::None,
                        NoiseSuppression::Standard,
                        NoiseSuppression::Neural,
                    ] {
                        ui.selectable_value(
                            &mut self.settings.noise_suppression_mode,
                            mode,
                            tr(mode.label()),
                        );
                    }
                });
            ui.separator();
            audio_toggle(ui, tr("Push To Talk"), &mut self.settings.push_to_talk);
            ui.add_enabled_ui(self.settings.push_to_talk, |ui| {
                let label = if self.capturing_ptt {
                    tr("Press a key…").to_owned()
                } else {
                    i18n::message(
                        "Hotkey: {key}",
                        &[("key", &audio::ptt_key_name(self.settings.push_to_talk_key))],
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
        if ui.button(tr("Refresh audio devices")).clicked() {
            self.inputs = audio::devices(true);
            self.outputs = audio::devices(false);
        }
    }
}
