use super::*;

impl Wizard {
    pub(super) fn audio_page(&mut self, ui: &mut egui::Ui, devices: &Devices<'_>) {
        ui.checkbox(&mut self.draft.desktop_audio, tr("PC audio"));
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(20.0);
        ui.checkbox(&mut self.draft.microphone, tr("Microphone"));
        ui.add_space(12.0);
        ui.add_enabled_ui(self.draft.microphone, |ui| {
            super::super::views::device_selector(
                ui,
                "setup-microphone",
                &mut self.draft.input_device,
                devices.inputs,
            );
        });
        ui.add_space(12.0);
        if self
            .mic_test
            .as_ref()
            .is_some_and(|test| !test.matches_settings(&self.draft))
            || !self.draft.microphone
        {
            self.mic_test = None;
        }
        self.microphone_test(ui);
    }

    fn microphone_test(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.draft.microphone, |ui| {
            ui.horizontal(|ui| {
                if ui.button(tr("Test microphone")).clicked() {
                    self.mic_test = Some(audio::MicTest::start(self.draft.clone()));
                }
                if let Some(test) = &self.mic_test {
                    match test.state() {
                        audio::MicTestState::Recording => {
                            if ui.button(tr("Listen back")).clicked() {
                                test.playback();
                            }
                        }
                        audio::MicTestState::Playing => {
                            if ui.button(tr("Stop playback")).clicked() {
                                test.stop_playback();
                            }
                        }
                        audio::MicTestState::Ready => {
                            if test.has_recording() && ui.button(tr("Listen again")).clicked() {
                                test.playback();
                            }
                        }
                    }
                }
            });
            let level = self.mic_test.as_ref().map_or(0.0, |test| test.level());
            ui.add(
                egui::ProgressBar::new(level)
                    .fill(ACCENT)
                    .desired_height(8.0),
            );
            if let Some(test) = &self.mic_test {
                if let Some(error) = test.error() {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                if test.state() != audio::MicTestState::Ready {
                    ui.ctx().request_repaint_after(Duration::from_millis(30));
                }
            }
        });
    }
}
