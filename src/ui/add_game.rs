use super::*;

#[derive(Default)]
pub(super) struct Dialog {
    name: String,
    executable: String,
    error: Option<String>,
    focused: bool,
}

impl Dialog {
    /// Return true after adding a valid profile or dismissing the dialog.
    pub fn show(&mut self, ctx: &egui::Context, settings: &mut Settings) -> bool {
        let response = egui::Modal::new(egui::Id::unique("add-custom-game"))
            .backdrop_color(Color32::from_black_alpha(180))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0_f32, LINE))
                    .corner_radius(14.0)
                    .inner_margin(26.0),
            )
            .show(ctx, |ui| {
                ui.set_width(420.0_f32.min((ctx.content_rect().width() - 80.0).max(200.0)));
                let close = self.header(ui);
                ui.add_space(20.0);
                self.fields(ui);
                if let Some(error) = &self.error {
                    ui.add_space(8.0);
                    ui.colored_label(DANGER, error);
                }
                ui.add_space(22.0);
                close || self.actions(ui, settings)
            });
        response.inner || response.should_close()
    }

    fn header(&self, ui: &mut egui::Ui) -> bool {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Add game")).size(23.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_sized(
                    [28.0, 28.0],
                    egui::Button::new(RichText::new("×").size(23.0)).frame_when_inactive(false),
                )
                .on_hover_text(tr("Cancel"))
                .clicked()
            })
            .inner
        })
        .inner
    }

    fn fields(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr("Game name")).strong());
        let name = ui.add(
            egui::TextEdit::singleline(&mut self.name)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(12, 10)),
        );
        if !self.focused {
            name.request_focus();
            self.focused = true;
        }
        ui.add_space(14.0);
        ui.label(RichText::new(tr("Executable")).strong());
        ui.add(
            egui::TextEdit::singleline(&mut self.executable)
                .hint_text("GameClient.exe")
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(12, 10)),
        );
    }

    fn actions(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> bool {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    !self.name.trim().is_empty() && !self.executable.trim().is_empty(),
                    egui::Button::new(RichText::new(tr("Add game")).color(TEXT))
                        .fill(ACCENT_DARK)
                        .min_size(Vec2::new(110.0, 36.0)),
                )
                .clicked()
            {
                match settings.add_game_profile(&self.name, &self.executable) {
                    Ok(()) => return true,
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
            ui.add_sized([90.0, 36.0], egui::Button::new(tr("Cancel")))
                .clicked()
        })
        .inner
    }
}
