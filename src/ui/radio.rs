//! Shared onboarding-style radio buttons.
use super::*;
use egui::Stroke;

pub(super) fn radio(
    ui: &mut egui::Ui,
    selected: bool,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().icon_width = 22.0;
        ui.spacing_mut().icon_width_inner = 14.0;
        ui.spacing_mut().icon_spacing = 10.0;
        let visuals = &mut ui.visuals_mut().widgets.inactive;
        visuals.bg_fill = if selected { ACCENT_DARK } else { BG };
        visuals.bg_stroke = Stroke::new(1.5, if selected { ACCENT } else { MUTED });
        visuals.fg_stroke = Stroke::new(1.5, ACCENT);
        ui.radio(selected, label)
    })
    .inner
}

pub(super) fn radio_value<T: PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    option: T,
    label: impl Into<egui::WidgetText>,
) -> egui::Response {
    let mut response = radio(ui, *value == option, label);
    if response.clicked() && *value != option {
        *value = option;
        response.mark_changed();
    }
    response
}
