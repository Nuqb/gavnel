use super::views::*;
use super::*;
use egui::{Align, Layout};
impl Gavnel {
    pub(super) fn settings_general(&mut self, ui: &mut egui::Ui) {
        section(ui, tr("Language"), |ui| {
            super::language::selector(ui, &mut self.settings.language, "settings-language");
        });
        ui.add_space(16.0);
        section(ui, tr("Clip feedback"), |ui| {
            ui.checkbox(
                &mut self.settings.clip_feedback.sound,
                tr("Play a sound when I clip"),
            );
            ui.add_enabled_ui(self.settings.clip_feedback.sound, |ui| {
                ui.label(RichText::new(tr("Sound volume")).size(12.0).color(MUTED));
                super::widgets::slider(ui, &mut self.settings.clip_feedback.volume, 0.0..=1.0, "%");
            });
            ui.add_space(8.0);
            ui.checkbox(
                &mut self.settings.clip_feedback.notification,
                tr("Show a clip notification"),
            );
            ui.label(RichText::new(tr("A small confirmation fades away after 3 seconds. Your game keeps focus and clicks pass through.")).size(12.0).color(MUTED));
            ui.add_space(10.0);
            if ui
                .add_enabled(
                    self.settings.clip_feedback.sound || self.settings.clip_feedback.notification,
                    egui::Button::new(tr("Preview clip feedback")),
                )
                .clicked()
            {
                self.send(Command::PreviewFeedback(
                    self.settings.clip_feedback.clone(),
                    self.settings.language,
                ));
            }
        });
        ui.add_space(16.0);
        section(ui, tr("Window & startup"), |ui| {
            ui.label(RichText::new(tr("When I close Gavnel")).strong());
            super::widgets::dropdown(
                ui,
                "close-behavior",
                &mut self.settings.close_to_tray,
                &[
                    (false, tr("Exit Gavnel")),
                    (true, tr("Close to system tray")),
                ],
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(tr("Start Gavnel in the tray with Windows"));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    super::widgets::toggle(ui, &mut self.settings.start_with_windows);
                });
            });
        });
        ui.add_space(16.0);
        section(ui, tr("Setup"), |ui| {
            ui.label(
                RichText::new(tr(
                    "Review the guided setup, or test it without changing your preferences.",
                ))
                .size(12.0)
                .color(MUTED),
            );
            ui.horizontal(|ui| {
                if ui.button(tr("Run setup again")).clicked() {
                    match self.apply() {
                        Ok(()) => self.begin_onboarding(),
                        Err(error) => self.message = error.to_string(),
                    }
                }
                if ui.button(tr("Preview setup")).clicked()
                    && let Err(error) = onboarding::open_preview()
                {
                    self.message = error.to_string();
                }
            });
        });
    }
}
