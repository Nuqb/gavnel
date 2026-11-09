use super::views::*;
use super::*;
use egui::{Align, Layout, Sense};
impl Gavnel {
    pub(super) fn settings_games(&mut self, ui: &mut egui::Ui) {
        section(ui, "Supported games", |ui| {
            let mut remove = None;
            for (i, p) in self.settings.profiles.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                    paint_icon(ui.painter(), r, Glyph::Game, MUTED);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&p.name).strong());
                        ui.label(RichText::new(&p.executable).size(11.0).color(MUTED));
                    });
                    if !p.is_official() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Remove").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                });
                ui.add_space(5.0);
            }
            if let Some(i) = remove {
                self.settings.profiles.remove(i);
            }
        });
        ui.add_space(14.0);
        section(ui, "Add a game", |ui| {
            description(
                ui,
                "Custom game profile",
                "Enter the program's executable filename, without a folder path.",
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.profile_name)
                        .hint_text("Game name")
                        .margin(egui::Margin::symmetric(12, 9))
                        .desired_width(180.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.profile_exe)
                        .hint_text("GameClient.exe")
                        .margin(egui::Margin::symmetric(12, 9))
                        .desired_width(240.0),
                );
            });
            if ui.button("Add game").clicked() {
                match self
                    .settings
                    .add_game_profile(&self.profile_name, &self.profile_exe)
                {
                    Ok(()) => {
                        self.profile_exe.clear();
                        self.profile_name.clear();
                    }
                    Err(error) => self.message = error.to_string(),
                }
            }
        });
    }
}
