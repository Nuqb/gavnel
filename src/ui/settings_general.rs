use super::views::*;
use super::*;
use egui::{Align, Layout};
impl Gavnel {
    pub(super) fn settings_general(&mut self, ui: &mut egui::Ui) {
        section(ui, "Clip feedback", |ui| {
            ui.checkbox(
                &mut self.settings.clip_feedback.sound,
                "Play a sound when I clip",
            );
            ui.add_enabled_ui(self.settings.clip_feedback.sound, |ui| {
                ui.label(RichText::new("Sound volume").size(12.0).color(MUTED));
                super::widgets::slider(ui, &mut self.settings.clip_feedback.volume, 0.0..=1.0, "%");
            });
            ui.add_space(8.0);
            ui.checkbox(
                &mut self.settings.clip_feedback.notification,
                "Show a clip notification",
            );
            ui.label(RichText::new("A small confirmation fades away after 3 seconds. Your game keeps focus and clicks pass through.").size(12.0).color(MUTED));
            ui.add_space(10.0);
            if ui
                .add_enabled(
                    self.settings.clip_feedback.sound || self.settings.clip_feedback.notification,
                    egui::Button::new("Preview clip feedback"),
                )
                .clicked()
            {
                self.send(Command::PreviewFeedback(
                    self.settings.clip_feedback.clone(),
                ));
            }
        });
        ui.add_space(16.0);
        section(ui, "Library", |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open capture folder").clicked() {
                    self.open_folder();
                }
                if ui.button("Organize existing clips").clicked() {
                    match self.library.organize(&self.settings.clips_dir) {
                        Ok(()) => self.message = "Organizing recordings…".into(),
                        Err(error) => self.message = error.to_string(),
                    }
                }
            });
        });
        ui.add_space(16.0);
        section(ui, "Window & startup", |ui| {
            ui.label(RichText::new("When I close Gavnel").strong());
            super::widgets::dropdown(
                ui,
                "close-behavior",
                &mut self.settings.close_to_tray,
                &[(false, "Exit Gavnel"), (true, "Close to system tray")],
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label("Start Gavnel in the tray with Windows");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    super::widgets::toggle(ui, &mut self.settings.start_with_windows);
                });
            });
        });
    }
}
