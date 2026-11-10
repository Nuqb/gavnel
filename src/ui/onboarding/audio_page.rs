use super::*;

impl Wizard {
    pub(super) fn audio_page(&mut self, ui: &mut egui::Ui, devices: &Devices<'_>) {
        controls::group(ui, tr("PC audio"), |ui| {
            self.output_selector(ui, devices.outputs);
        });
        ui.add_space(8.0);
        controls::group(ui, tr("Microphone"), |ui| {
            self.input_selector(ui, devices.inputs);
            ui.add_space(8.0);
            if !self.draft.microphone
                || self
                    .mic_test
                    .as_ref()
                    .is_some_and(|test| !test.matches_settings(&self.draft))
            {
                self.mic_test = None;
            }
            ui.add_enabled_ui(self.draft.microphone, |ui| {
                super::super::microphone_test::show(ui, &self.draft, &mut self.mic_test);
            });
        });
    }

    fn output_selector(&mut self, ui: &mut egui::Ui, devices: &[AudioDevice]) {
        let label = if !self.draft.desktop_audio {
            tr("Off")
        } else {
            match &self.draft.output_devices {
                None => tr("All PC audio"),
                Some(ids) if ids.len() == 1 => devices
                    .iter()
                    .find(|d| d.id == ids[0])
                    .map_or(tr("Selected device is disconnected"), |d| d.label.as_str()),
                Some(_) => tr("Selected playback devices"),
            }
        };
        super::super::motion::ComboBox::from_id_salt("setup-output")
            .width(ui.available_width())
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.draft.desktop_audio, false, tr("Off"));
                if ui
                    .selectable_label(
                        self.draft.desktop_audio && self.draft.output_devices.is_none(),
                        tr("All PC audio"),
                    )
                    .clicked()
                {
                    self.draft.desktop_audio = true;
                    self.draft.output_devices = None;
                }
                for device in devices {
                    let selected = self.draft.desktop_audio
                        && self
                            .draft
                            .output_devices
                            .as_ref()
                            .is_some_and(|ids| ids.as_slice() == [device.id.clone()]);
                    if ui.selectable_label(selected, &device.label).clicked() {
                        self.draft.desktop_audio = true;
                        self.draft.output_devices = Some(vec![device.id.clone()]);
                    }
                }
            });
    }

    fn input_selector(&mut self, ui: &mut egui::Ui, devices: &[AudioDevice]) {
        let label = if !self.draft.microphone {
            tr("Off")
        } else if self.draft.input_device.is_empty() {
            tr("Windows default device")
        } else {
            devices
                .iter()
                .find(|d| d.id == self.draft.input_device)
                .map_or(tr("Selected device is disconnected"), |d| d.label.as_str())
        };
        super::super::motion::ComboBox::from_id_salt("setup-microphone")
            .width(ui.available_width())
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.draft.microphone, false, tr("Off"));
                if ui
                    .selectable_label(
                        self.draft.microphone && self.draft.input_device.is_empty(),
                        tr("Windows default device"),
                    )
                    .clicked()
                {
                    self.draft.microphone = true;
                    self.draft.input_device.clear();
                }
                for device in devices {
                    if ui
                        .selectable_label(
                            self.draft.microphone && self.draft.input_device == device.id,
                            &device.label,
                        )
                        .clicked()
                    {
                        self.draft.microphone = true;
                        self.draft.input_device = device.id.clone();
                    }
                }
            });
    }
}
