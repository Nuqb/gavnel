//! Shared desktop typography, colors, and widget spacing.
use super::*;
pub(super) fn apply(ctx: &egui::Context) {
    ctx.on_end_pass(
        "clickable-cursor",
        Arc::new(|ui| widgets::clickable_cursor(ui.ctx())),
    );
    ctx.set_fonts(super::fonts::definitions());
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(34, 34, 38);
    visuals.faint_bg_color = PANEL;
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, LINE);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, MUTED);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, LINE);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(6);
    }
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 44, 41);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(48, 44, 41);
    visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0_f32, Color32::from_rgb(255, 184, 112));
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5_f32, TEXT);
    visuals.widgets.active.bg_fill = ACCENT_DARK;
    visuals.widgets.active.weak_bg_fill = ACCENT_DARK;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5_f32, TEXT);
    visuals.widgets.open.bg_fill = CARD;
    visuals.widgets.open.weak_bg_fill = Color32::from_rgb(42, 37, 33);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.open.fg_stroke = egui::Stroke::new(1.5_f32, TEXT);
    visuals.selection.bg_fill = ACCENT_DARK;
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.slider_trailing_fill = true;
    ctx.set_visuals(visuals);
    ctx.style_mut_of(egui::Theme::Dark, |s| {
        s.spacing.item_spacing = Vec2::new(10.0, 10.0);
        s.spacing.button_padding = Vec2::new(14.0, 9.0);
        s.spacing.interact_size = Vec2::new(44.0, 36.0);
        s.spacing.slider_width = 240.0;
        s.spacing.slider_rail_height = 5.0;
        s.spacing.menu_margin = egui::Margin::same(8);
        s.spacing.combo_width = 220.0;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(23.0));
    });
}
