use super::*;
#[derive(Clone, Copy)]
pub(super) enum Icon {
    Trim,
    Crop,
    Audio,
    Undo,
    Redo,
    Reset,
    Center,
    Play,
    Pause,
    SaveVideo,
}
impl Icon {
    fn image(self, ui: &egui::Ui) -> egui::Image<'static> {
        let (name, svg): (&'static str, &[u8]) = match self {
            Self::SaveVideo => (
                "project-save",
                include_bytes!("../../../assets/icons/project-save.svg"),
            ),
            Self::Trim => (
                "editor-trim",
                include_bytes!("../../../assets/icons/editor-trim.svg"),
            ),
            Self::Crop => (
                "editor-crop",
                include_bytes!("../../../assets/icons/editor-crop.svg"),
            ),
            Self::Undo => (
                "editor-undo",
                include_bytes!("../../../assets/icons/editor-undo.svg"),
            ),
            Self::Redo => (
                "editor-redo",
                include_bytes!("../../../assets/icons/editor-redo.svg"),
            ),
            Self::Reset => (
                "editor-reset",
                include_bytes!("../../../assets/icons/editor-reset.svg"),
            ),
            Self::Center => (
                "editor-center",
                include_bytes!("../../../assets/icons/editor-center.svg"),
            ),
            Self::Audio => (
                "editor-audio",
                include_bytes!("../../../assets/icons/volume.svg"),
            ),
            Self::Play => (
                "editor-play",
                include_bytes!("../../../assets/icons/play.svg"),
            ),
            Self::Pause => (
                "editor-pause",
                include_bytes!("../../../assets/icons/pause.svg"),
            ),
        };
        let texture = crate::icons::texture(ui.ctx(), name, svg, 32);
        egui::Image::new((texture.id(), Vec2::splat(20.0))).tint(TEXT)
    }
}
pub(super) fn button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    ui.add(
        egui::Button::image(icon.image(ui))
            .frame_when_inactive(false)
            .stroke(egui::Stroke::NONE),
    )
    .on_hover_text(tr(label))
}
pub(super) fn enabled(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::image(icon.image(ui))
            .frame_when_inactive(false)
            .stroke(egui::Stroke::NONE),
    )
    .on_hover_text(tr(label))
}
pub(super) fn tab(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool) -> egui::Response {
    ui.add(
        egui::Button::image(icon.image(ui))
            .frame_when_inactive(false)
            .stroke(egui::Stroke::NONE)
            .selected(selected),
    )
    .on_hover_text(tr(label))
}
