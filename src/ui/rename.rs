use super::*;
use std::path::Path;

pub(super) struct Dialog {
    path: PathBuf,
    name: String,
    error: Option<String>,
    focused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    None,
    Cancel,
    Rename,
}

impl Dialog {
    pub(super) fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            error: None,
            focused: false,
        }
    }

    fn show(&mut self, ctx: &egui::Context) -> Action {
        let response = egui::Modal::new(egui::Id::unique("rename-capture"))
            .backdrop_color(Color32::from_black_alpha(180))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .corner_radius(14.0)
                    .inner_margin(26.0),
            )
            .show(ctx, |ui| {
                ui.set_width(420.0_f32.min((ctx.content_rect().width() - 80.0).max(200.0)));
                ui.label(RichText::new(tr("Rename")).size(23.0).strong());
                ui.add_space(18.0);
                let enter = self.field(ui);
                if let Some(error) = &self.error {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(RichText::new(error).color(DANGER)).wrap());
                }
                ui.add_space(20.0);
                self.actions(ui, enter)
            });
        if response.should_close() {
            Action::Cancel
        } else {
            response.inner
        }
    }

    fn field(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label(tr("Name"));
        let extension = format!(
            ".{}",
            self.path.extension().unwrap_or_default().to_string_lossy()
        );
        let response = ui
            .horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .desired_width((ui.available_width() - 50.0).max(100.0))
                        .margin(egui::Margin::symmetric(12, 10)),
                );
                ui.label(extension);
                field
            })
            .inner;
        if !self.focused {
            response.request_focus();
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(self.name.chars().count()),
                    )));
                state.store(ui.ctx(), response.id);
            }
            self.focused = true;
        }
        if response.changed() {
            self.error = None;
        }
        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
    }

    fn actions(&self, ui: &mut egui::Ui, enter: bool) -> Action {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let enabled = !self.name.trim().is_empty();
            let rename = ui
                .add_enabled(
                    enabled,
                    egui::Button::new(tr("Rename"))
                        .fill(ACCENT_DARK)
                        .min_size(Vec2::new(104.0, 36.0)),
                )
                .clicked();
            if ui
                .add_sized([90.0, 36.0], egui::Button::new(tr("Cancel")))
                .clicked()
            {
                Action::Cancel
            } else if enabled && (rename || enter) {
                Action::Rename
            } else {
                Action::None
            }
        })
        .inner
    }
}

impl Gavnel {
    pub(super) fn show_rename_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.rename_dialog.take() else {
            return;
        };
        match dialog.show(ctx) {
            Action::Cancel => return,
            Action::Rename => {
                let result = if self.saves.status(&dialog.path).is_some() {
                    Err(anyhow::anyhow!("Wait for this capture to finish saving."))
                } else {
                    crate::library::rename_capture(&dialog.path, &dialog.name)
                };
                match result {
                    Ok(path) => {
                        self.capture_renamed(&dialog.path, &path);
                        return;
                    }
                    Err(error) => dialog.error = Some(format!("{error:#}")),
                }
            }
            Action::None => {}
        }
        self.rename_dialog = Some(dialog);
    }

    fn capture_renamed(&mut self, old: &Path, new: &Path) {
        if let Some(texture) = self.textures.remove(old) {
            self.textures.insert(new.to_owned(), texture);
        }
        self.thumbnails.invalidate(old);
        self.thumbnails.invalidate(new);
        for selected in &mut self.montage_selection {
            if selected == old {
                *selected = new.to_owned();
            }
        }
        if let Some(clip) = self
            .library
            .clips
            .lock()
            .unwrap()
            .iter_mut()
            .find(|c| c.path == old)
        {
            clip.path = new.to_owned();
            clip.name = new
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .replace('_', " ");
        }
        self.library.refresh(&self.settings.clips_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, dialog: &mut Dialog, events: Vec<egui::Event>) -> Action {
        let mut action = Action::None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(960.0, 640.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                action = dialog.show(ctx);
            },
        );
        output.textures_delta.clear();
        action
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn name_is_selected_for_replacement_and_enter_submits() {
        let ctx = egui::Context::default();
        let mut dialog = Dialog::new(Path::new("old.mp4"));
        for _ in 0..3 {
            assert_eq!(frame(&ctx, &mut dialog, vec![]), Action::None);
        }
        frame(
            &ctx,
            &mut dialog,
            vec![egui::Event::Text("Highlight".into())],
        );
        assert_eq!(dialog.name, "Highlight");
        assert_eq!(
            frame(&ctx, &mut dialog, vec![key(egui::Key::Enter)]),
            Action::Rename
        );
        assert_eq!(
            dialog.path,
            Path::new("old.mp4"),
            "The dialog must not touch files"
        );
    }

    #[test]
    fn escape_cancels_and_empty_name_cannot_submit() {
        let ctx = egui::Context::default();
        let mut dialog = Dialog::new(Path::new("old.PNG"));
        for _ in 0..3 {
            frame(&ctx, &mut dialog, vec![]);
        }
        frame(&ctx, &mut dialog, vec![key(egui::Key::Backspace)]);
        assert!(dialog.name.is_empty());
        assert_eq!(
            frame(&ctx, &mut dialog, vec![key(egui::Key::Enter)]),
            Action::None
        );
        assert_eq!(
            frame(&ctx, &mut dialog, vec![key(egui::Key::Escape)]),
            Action::Cancel
        );
    }
}
