use super::*;

impl Wizard {
    pub(super) fn preferences_page(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        controls::group(ui, tr("Save location"), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(tr("Browse…")).clicked() {
                        *action = Some(Action::BrowseFolder);
                    }
                    ui.add_sized(
                        [ui.available_width(), 36.0],
                        egui::TextEdit::singleline(&mut self.folder),
                    );
                });
            });
        });
        controls::group(ui, tr("App behavior"), |ui| {
            super::super::checkbox::checkbox(
                ui,
                &mut self.draft.start_with_windows,
                tr("Open on Windows startup"),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(tr("When I press X"));
                super::super::widgets::dropdown(
                    ui,
                    "setup-close",
                    &mut self.draft.close_to_tray,
                    &[(false, tr("Exit Gavnel")), (true, tr("Minimize to tray"))],
                );
            });
        });
        controls::group(ui, tr("Clip feedback"), |ui| {
            ui.columns(2, |columns| {
                super::super::checkbox::checkbox(
                    &mut columns[0],
                    &mut self.draft.clip_feedback.notification,
                    tr("Notification"),
                );
                super::super::checkbox::checkbox(
                    &mut columns[1],
                    &mut self.draft.clip_feedback.sound,
                    tr("Sound"),
                );
            });
        });
    }

    pub(super) fn review(&mut self, ui: &mut egui::Ui) {
        let source = match self.draft.mode {
            CaptureMode::Automatic => tr("Automatic game detection").to_owned(),
            CaptureMode::Monitor => i18n::message(
                "Screen {number}",
                &[("number", &self.draft.monitor_index.to_string())],
            ),
            CaptureMode::Window => self.draft.window_title.clone(),
        };
        let value_width = (ui.available_width() - 164.0).max(100.0);
        egui::Grid::new("setup-summary")
            .min_row_height(20.0)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                for (label, value) in [
                    (tr("Capture"), source),
                    (
                        tr("Quality"),
                        format!(
                            "{}p · {} FPS · {} Mbps",
                            self.draft.height, self.draft.fps, self.draft.bitrate_mbps
                        ),
                    ),
                    (
                        tr("Replay"),
                        i18n::message(
                            "{seconds} s",
                            &[("seconds", &self.draft.replay_seconds.to_string())],
                        ),
                    ),
                    (
                        tr("Audio"),
                        format!(
                            "{}: {} · {}: {}",
                            tr("PC audio"),
                            on_off(self.draft.desktop_audio),
                            tr("Microphone"),
                            on_off(self.draft.microphone)
                        ),
                    ),
                    (tr("Startup"), on_off(self.draft.start_with_windows).into()),
                    (
                        tr("Close button"),
                        if self.draft.close_to_tray {
                            tr("Minimize to tray")
                        } else {
                            tr("Exit Gavnel")
                        }
                        .into(),
                    ),
                ] {
                    ui.add_sized(
                        [140.0, 20.0],
                        egui::Label::new(RichText::new(label).color(MUTED)).wrap(),
                    );
                    ui.add_sized([value_width, 20.0], egui::Label::new(value).truncate());
                    ui.end_row();
                }
                ui.add_sized(
                    [140.0, 20.0],
                    egui::Label::new(RichText::new(tr("Save location")).color(MUTED)).wrap(),
                );
                ui.add_sized(
                    [value_width, 20.0],
                    egui::Label::new(&self.folder).truncate(),
                )
                .on_hover_text(&self.folder);
                ui.end_row();
            });
        ui.add_space(16.0);
        super::super::checkbox::checkbox(
            ui,
            &mut self.start_capture,
            tr("Start capture after setup"),
        );
    }
}

fn on_off(value: bool) -> &'static str {
    if value { tr("On") } else { tr("Off") }
}
