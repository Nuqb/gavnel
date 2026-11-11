use super::*;

pub(super) fn show(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let texture = crate::icons::texture(
        ui.ctx(),
        "back",
        include_bytes!("../../assets/icons/back.svg"),
        32,
    );
    ui.add(
        egui::Button::image_and_text(
            egui::Image::new((texture.id(), Vec2::splat(18.0))).tint(TEXT),
            RichText::new(tr(label)).size(16.0).color(TEXT),
        )
        .frame_when_inactive(false)
        .stroke(egui::Stroke::NONE),
    )
}
