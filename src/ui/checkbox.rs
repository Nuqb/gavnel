//! App-wide checkboxes and checkmarks. Use these for every checked control or status.
use super::*;
use egui::Stroke;

pub(super) fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    let width = ui.available_width();
    let text = egui::WidgetText::from(label).into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        (width - 34.0).max(1.0),
        egui::TextStyle::Body,
    );
    let (rect, mut response) = ui.allocate_exact_size(
        Vec2::new(
            if label.is_empty() {
                22.0
            } else {
                (text.size().x + 34.0).min(width)
            },
            text.size().y.max(30.0),
        ),
        egui::Sense::click(),
    );
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), *value, label)
    });
    let icon_x = if i18n::is_rtl() {
        rect.right() - 22.0
    } else {
        rect.left()
    };
    let icon = egui::Rect::from_min_size(
        egui::pos2(icon_x, rect.center().y - 11.0),
        Vec2::splat(22.0),
    );
    paint_checkbox(ui, icon, *value, &response);
    let text_x = if i18n::is_rtl() {
        icon.left() - 12.0 - text.size().x
    } else {
        icon.right() + 12.0
    };
    ui.painter().galley(
        egui::pos2(text_x, rect.center().y - text.size().y * 0.5),
        text,
        if ui.is_enabled() { TEXT } else { MUTED },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_checkbox(ui: &egui::Ui, rect: egui::Rect, checked: bool, response: &egui::Response) {
    let active = response.hovered() || response.has_focus();
    let accent = if ui.is_enabled() { ACCENT } else { MUTED };
    let fill = if checked {
        accent
    } else if active {
        ACCENT_DARK
    } else {
        BG
    };
    let border = if checked || active {
        accent
    } else {
        MUTED.gamma_multiply(0.65)
    };
    ui.painter().rect(
        rect,
        6.0,
        fill,
        Stroke::new(1.5, border),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(3.0),
            9.0,
            Stroke::new(1.0, accent),
            egui::StrokeKind::Outside,
        );
    }
    if checked {
        paint_checkmark(ui.painter(), rect, BG);
    }
}

/// The onboarding tick, shared by controls and noninteractive selection indicators.
fn paint_checkmark(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let scale = rect.width() / 22.0;
    let points = [
        rect.min + Vec2::new(5.5, 11.0) * scale,
        rect.min + Vec2::new(9.5, 15.0) * scale,
        rect.min + Vec2::new(16.5, 7.0) * scale,
    ];
    painter.add(egui::Shape::line(
        points.to_vec(),
        Stroke::new(2.5 * scale, color),
    ));
    for point in points {
        painter.circle_filled(point, 1.25 * scale, color);
    }
}

pub(super) fn indicator(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
    paint_checkbox(ui, rect, true, &response);
    response
}
#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        value: &mut bool,
        enabled: bool,
        events: Vec<egui::Event>,
    ) -> egui::Response {
        let mut response = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    response = Some(
                        ui.add_enabled_ui(enabled, |ui| checkbox(ui, value, "Notification"))
                            .inner,
                    );
                });
            },
        );
        output.textures_delta.clear();
        response.unwrap()
    }

    #[test]
    fn custom_checkbox_supports_keyboard_toggle_and_disabled_state() {
        let ctx = egui::Context::default();
        crate::ui::theme::apply(&ctx);
        let mut value = false;
        let response = frame(&ctx, &mut value, true, Vec::new());
        ctx.memory_mut(|memory| memory.request_focus(response.id));
        let key = |pressed| egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(&ctx, &mut value, true, vec![key(true)]);
        frame(&ctx, &mut value, true, vec![key(false)]);
        assert!(value, "Space must toggle the focused checkbox");
        frame(&ctx, &mut value, false, vec![key(true)]);
        frame(&ctx, &mut value, false, vec![key(false)]);
        assert!(value, "A disabled checkbox must preserve its value");
    }
}
