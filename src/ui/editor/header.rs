use super::*;

impl Editor {
    pub(super) fn header(&mut self, ui: &mut egui::Ui, app: &mut Gavnel) {
        ui.horizontal(|ui| {
            if super::super::back_button::show(
                ui,
                if self.return_player.is_some() {
                    "Player"
                } else {
                    "Library"
                },
            )
            .clicked()
            {
                app.page = if self.return_player.is_some() {
                    Page::Player
                } else {
                    Page::Clips
                };
                app.player = None;
            }
            if let Some(segment) = self
                .project
                .as_ref()
                .and_then(|p| p.segments.get(self.selected))
            {
                let name = segment
                    .info
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let name_width = (ui.available_width() - 260.0).clamp(0.0, 320.0);
                ui.add_space(8.0);
                ui.scope(|ui| {
                    ui.set_max_width(name_width);
                    ui.add(
                        egui::Label::new(RichText::new(name.as_ref()).size(16.0).strong())
                            .truncate(),
                    )
                    .on_hover_text(name.as_ref());
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if buttons::enabled(
                    ui,
                    buttons::Icon::SaveVideo,
                    "Save video",
                    self.project.is_some() && !self.loading,
                )
                .clicked()
                {
                    app.player = None;
                    self.save_editing = false;
                    self.save_open = true;
                }
                if buttons::enabled(ui, buttons::Icon::Redo, "Redo", !self.redo.is_empty())
                    .on_hover_text("Ctrl+Shift+Z")
                    .clicked()
                {
                    self.redo();
                    app.player = None;
                }
                if buttons::enabled(ui, buttons::Icon::Undo, "Undo", !self.undo.is_empty())
                    .on_hover_text("Ctrl+Z")
                    .clicked()
                {
                    self.undo();
                    app.player = None;
                }
            });
        });
    }
}
