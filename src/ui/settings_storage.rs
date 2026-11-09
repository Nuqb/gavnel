use super::views::*;
use super::*;
use egui::{Align, Layout, Pos2, Rect, Sense};
impl Gavnel {
    pub(super) fn settings_storage(&mut self, ui: &mut egui::Ui) {
        section(ui, tr("Storage Settings"), |ui| {
            ui.separator();
            let available = ui.available_width();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((available - 210.0).max(300.0));
                    ui.label(RichText::new(tr("Capture Folder Location")).strong());
                    ui.horizontal(|ui| {
                        let width = ui.available_width() - 100.0;
                        let field = super::widgets::field(ui, &mut self.storage_text, width);
                        if field.lost_focus()
                            && self.storage_text.trim() != self.settings.clips_dir.to_string_lossy()
                            && let Err(error) =
                                self.change_clip_folder(PathBuf::from(self.storage_text.trim()))
                        {
                            self.message = error.to_string();
                        }
                        if ui
                            .add_sized([88.0, 38.0], egui::Button::new(tr("Browse")))
                            .clicked()
                        {
                            self.browse_clip_folder();
                        }
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(tr("Local Storage Limit")).strong());
                        super::motion::ComboBox::from_id_salt("storage-limit")
                            .selected_text(
                                self.settings
                                    .storage_limit_gb
                                    .map_or(tr("No Limit").to_owned(), |gb| format!("{gb} GB")),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.settings.storage_limit_gb,
                                    None,
                                    tr("No Limit"),
                                );
                                for gb in [10, 25, 50, 100, 250, 500, 1000] {
                                    ui.selectable_value(
                                        &mut self.settings.storage_limit_gb,
                                        Some(gb),
                                        format!("{gb} GB"),
                                    );
                                }
                            });
                    });
                });
            });
            ui.add_space(24.0);
            match crate::storage::drive_space(&self.settings.clips_dir) {
                Ok(space) => {
                    let clips: u64 = self
                        .library
                        .clips
                        .lock()
                        .unwrap()
                        .iter()
                        .map(|clip| clip.bytes)
                        .sum();
                    let other = space.total.saturating_sub(space.free).saturating_sub(clips);
                    ui.label(
                        RichText::new(i18n::message("Drive {name}", &[("name", &space.root)]))
                            .strong(),
                    );
                    ui.label(i18n::message(
                        "{free} of {total} available",
                        &[
                            ("free", &crate::storage::format_bytes(space.free)),
                            ("total", &crate::storage::format_bytes(space.total)),
                        ],
                    ));
                    let (rect, _) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 25.0), Sense::hover());
                    ui.painter().rect_filled(rect, 5.0, BG);
                    let mut left = rect.left();
                    for (bytes, color) in [(clips, ACCENT), (other, Color32::from_gray(75))] {
                        let width =
                            rect.width() * (bytes as f64 / space.total.max(1) as f64) as f32;
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                Pos2::new(left, rect.top()),
                                Vec2::new(width, rect.height()),
                            ),
                            2.0,
                            color,
                        );
                        left += width;
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (label, bytes, color) in [
                            (tr("Clips"), clips, ACCENT),
                            (tr("Other"), other, Color32::from_gray(100)),
                            (tr("Available space"), space.free, MUTED),
                        ] {
                            ui.colored_label(color, "■");
                            ui.label(format!("{label} {}", crate::storage::format_bytes(bytes)));
                            ui.add_space(8.0);
                        }
                    });
                }
                Err(error) => {
                    ui.colored_label(DANGER, error.to_string());
                }
            }
        });
        ui.add_space(14.0);
        if ui.button(tr("Organize existing clips")).clicked() {
            match self.library.organize(&self.settings.clips_dir) {
                Ok(()) => self.message = tr("Organizing recordings…").into(),
                Err(error) => self.message = error.to_string(),
            }
        }
    }
}
