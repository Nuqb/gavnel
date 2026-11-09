use super::*;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    None,
    Cancel,
    Delete,
}

/// View only: callers perform deletion after an explicit Delete button click.
pub(super) fn show(
    ctx: &egui::Context,
    path: &Path,
    details: Option<&crate::library::Clip>,
    error: Option<&str>,
) -> Action {
    let id = egui::Id::new("delete-recording-confirmation");
    let screenshot = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"));
    let response = egui::Modal::new(id)
        .backdrop_color(Color32::from_black_alpha(180))
        .frame(
            egui::Frame::new()
                .fill(PANEL)
                .stroke(egui::Stroke::new(1.0_f32, LINE))
                .corner_radius(16.0)
                .inner_margin(26.0)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 14],
                    blur: 36,
                    spread: 0,
                    color: Color32::from_black_alpha(110),
                }),
        )
        .show(ctx, |ui| {
            ui.set_width(420.0);
            let mut action = Action::None;
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(46.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 12.0, DANGER_DARK);
                crate::icons::paint(
                    ui.painter(),
                    "delete-recording",
                    include_bytes!("../../assets/icons/trash.svg"),
                    egui::Rect::from_center_size(rect.center(), Vec2::splat(24.0)),
                    DANGER,
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new(if screenshot {
                        "Delete this screenshot?"
                    } else {
                        "Delete this clip?"
                    })
                    .size(22.0)
                    .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [28.0, 28.0],
                            egui::Button::new(RichText::new("×").size(23.0).color(MUTED))
                                .frame_when_inactive(false),
                        )
                        .on_hover_text("Cancel")
                        .clicked()
                    {
                        action = Action::Cancel;
                    }
                });
            });
            ui.add_space(16.0);
            ui.add(
                egui::Label::new(
                    RichText::new("This capture will be permanently deleted from your computer.")
                        .color(MUTED),
                )
                .wrap(),
            );
            ui.add_space(18.0);
            egui::Frame::new()
                .fill(CARD)
                .corner_radius(9.0)
                .inner_margin(14.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(details.map(|c| c.source.as_str()).unwrap_or("Recording"))
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                .size(13.0)
                                .strong(),
                        )
                        .wrap(),
                    );
                    if let Some(clip) = details {
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(format!(
                                "{}:{:02}  ·  {}",
                                clip.duration.max(0.0) as u64 / 60,
                                clip.duration.max(0.0) as u64 % 60,
                                crate::storage::format_bytes(clip.bytes)
                            ))
                            .size(12.0)
                            .color(MUTED),
                        );
                    }
                });
            if let Some(error) = error {
                ui.add_space(12.0);
                ui.add(egui::Label::new(RichText::new(error).size(12.0).color(DANGER)).wrap());
            }
            ui.add_space(23.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::destructive_button(
                        ui,
                        if screenshot {
                            "Delete screenshot"
                        } else {
                            "Delete clip"
                        },
                        true,
                    )
                    .clicked()
                    {
                        action = Action::Delete;
                    }
                    let cancel = ui.add_sized(
                        [104.0, 40.0],
                        egui::Button::new("Cancel").fill(CARD).corner_radius(7.0),
                    );
                    if cancel.clicked() {
                        action = Action::Cancel;
                    }
                });
            });
            action
        });
    if response.inner == Action::None && response.should_close() {
        Action::Cancel
    } else {
        response.inner
    }
}
