//! Integrated window controls and a draggable toolbar background.
use super::*;
use egui::{Rect, Sense, Stroke, StrokeKind, ViewportCommand};

const BUTTON_WIDTH: f32 = 44.0;

pub(super) fn resize(root: &mut egui::Ui) {
    use egui::{CursorIcon, ResizeDirection};
    let rect = root.max_rect();
    let Some(pos) = root.input(|input| {
        (!input.viewport().maximized.unwrap_or(false))
            .then(|| input.pointer.hover_pos())
            .flatten()
    }) else {
        return;
    };
    if !rect.contains(pos) {
        return;
    }
    let left = pos.x < rect.left() + 5.0;
    let right = pos.x > rect.right() - 5.0;
    let top = pos.y < rect.top() + 5.0;
    let bottom = pos.y > rect.bottom() - 5.0;
    let (direction, cursor) = match (left, right, top, bottom) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::ResizeNwSe),
        (_, true, true, _) => (ResizeDirection::NorthEast, CursorIcon::ResizeNeSw),
        (true, _, _, true) => (ResizeDirection::SouthWest, CursorIcon::ResizeNeSw),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::ResizeNwSe),
        (true, _, _, _) => (ResizeDirection::West, CursorIcon::ResizeHorizontal),
        (_, true, _, _) => (ResizeDirection::East, CursorIcon::ResizeHorizontal),
        (_, _, true, _) => (ResizeDirection::North, CursorIcon::ResizeVertical),
        (_, _, _, true) => (ResizeDirection::South, CursorIcon::ResizeVertical),
        _ => return,
    };
    root.ctx().set_cursor_icon(cursor);
    if root.input(|input| input.pointer.primary_pressed()) {
        root.ctx()
            .send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

pub(super) fn show(root: &mut egui::Ui, height: f32, content: impl FnOnce(&mut egui::Ui)) {
    egui::Panel::top("recording-toolbar")
        .exact_size(height)
        .frame(egui::Frame::new().fill(PANEL))
        .show(root, |ui| {
            let rect = ui.max_rect();
            // Register the background first so controls and popups take precedence.
            let drag = ui.interact(
                rect,
                ui.scope_id().with("window-drag"),
                Sense::click_and_drag(),
            );
            if drag.double_clicked() {
                toggle_maximized(ui.ctx());
            } else if drag.drag_started_by(egui::PointerButton::Primary) {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
            let controls_left = rect.right() - 3.0 * BUTTON_WIDTH;
            let content_rect = Rect::from_min_max(
                rect.min + egui::vec2(20.0, 12.0),
                egui::pos2(controls_left - 12.0, rect.bottom() - 12.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), content);
            controls(
                ui,
                Rect::from_min_max(egui::pos2(controls_left, rect.top()), rect.max),
            );
        });
}

fn toggle_maximized(ctx: &egui::Context) {
    let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
}

fn controls(ui: &mut egui::Ui, rect: Rect) {
    let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
    for (index, label) in [
        "Minimize",
        if maximized { "Restore" } else { "Maximize" },
        "Close",
    ]
    .into_iter()
    .enumerate()
    {
        let button = Rect::from_min_size(
            rect.min + egui::vec2(index as f32 * BUTTON_WIDTH, 0.0),
            egui::vec2(BUTTON_WIDTH, rect.height()),
        );
        let response = ui.interact(
            button,
            ui.scope_id().with(("window-control", index)),
            Sense::click(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(label)));
        if response.hovered() || response.has_focus() {
            ui.painter()
                .rect_filled(button, 0.0, if index == 2 { DANGER_DARK } else { CARD });
        }
        paint_control(ui.painter(), button.center(), index, maximized);
        if response.on_hover_text(tr(label)).clicked() {
            match index {
                0 => ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true)),
                1 => toggle_maximized(ui.ctx()),
                _ => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
            }
        }
    }
}

fn paint_control(painter: &egui::Painter, center: egui::Pos2, index: usize, maximized: bool) {
    let stroke = Stroke::new(1.2, TEXT);
    let icon = Rect::from_center_size(center, Vec2::splat(10.0));
    match index {
        0 => {
            painter.line_segment([icon.left_center(), icon.right_center()], stroke);
        }
        1 => {
            if maximized {
                let back = icon.translate(egui::vec2(2.0, -2.0));
                painter.rect_stroke(back, 0.0, stroke, StrokeKind::Inside);
                painter.rect_filled(icon, 0.0, PANEL);
            }
            painter.rect_stroke(icon, 0.0, stroke, StrokeKind::Inside);
        }
        _ => {
            painter.line_segment([icon.left_top(), icon.right_bottom()], stroke);
            painter.line_segment([icon.right_top(), icon.left_bottom()], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, time: f64, events: Vec<egui::Event>) -> Vec<ViewportCommand> {
        let output = super::super::test_render::run(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 650.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |root| {
                show(root, 66.0, |ui| {
                    let _ = ui.button("Recording");
                })
            },
        );
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .clone()
    }

    fn click(ctx: &egui::Context, time: f64, pos: egui::Pos2) -> Vec<ViewportCommand> {
        frame(
            ctx,
            time,
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        );
        frame(ctx, time + 0.02, vec![pointer(pos, false)])
    }

    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn borderless_edges_resize_only_when_restored() {
        use egui::ResizeDirection::*;
        for (pos, direction) in [
            (egui::pos2(1.0, 1.0), NorthWest),
            (egui::pos2(999.0, 1.0), NorthEast),
            (egui::pos2(1.0, 649.0), SouthWest),
            (egui::pos2(999.0, 649.0), SouthEast),
            (egui::pos2(1.0, 300.0), West),
            (egui::pos2(999.0, 300.0), East),
            (egui::pos2(500.0, 1.0), North),
            (egui::pos2(500.0, 649.0), South),
        ] {
            for maximized in [false, true] {
                let ctx = egui::Context::default();
                let mut input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 650.0),
                    )),
                    events: vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
                    ..Default::default()
                };
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .maximized = Some(maximized);
                let output = super::super::test_render::run(&ctx, input, resize);
                assert_eq!(
                    output.viewport_output[&egui::ViewportId::ROOT]
                        .commands
                        .contains(&ViewportCommand::BeginResize(direction)),
                    !maximized
                );
            }
        }
    }

    #[test]
    fn window_buttons_send_native_commands() {
        for (x, command) in [
            (890.0, ViewportCommand::Minimized(true)),
            (934.0, ViewportCommand::Maximized(true)),
            (978.0, ViewportCommand::Close),
        ] {
            let ctx = egui::Context::default();
            frame(&ctx, 0.0, vec![]);
            let commands = click(&ctx, 0.1, egui::pos2(x, 30.0));
            assert!(commands.contains(&command), "{commands:?}");
            assert!(!commands.contains(&ViewportCommand::StartDrag));
        }
    }

    #[test]
    fn empty_toolbar_drags_and_double_clicks_but_recording_button_does_not() {
        let ctx = egui::Context::default();
        frame(&ctx, 0.0, vec![]);
        let pos = egui::pos2(600.0, 30.0);
        frame(
            &ctx,
            0.1,
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        );
        let commands = frame(
            &ctx,
            0.15,
            vec![egui::Event::PointerMoved(pos + egui::vec2(25.0, 0.0))],
        );
        assert!(commands.contains(&ViewportCommand::StartDrag));
        frame(&ctx, 0.2, vec![pointer(pos, false)]);
        click(&ctx, 1.0, pos);
        assert!(click(&ctx, 1.15, pos).contains(&ViewportCommand::Maximized(true)));
        let button = egui::pos2(50.0, 30.0);
        click(&ctx, 2.0, button);
        assert!(!click(&ctx, 2.15, button).contains(&ViewportCommand::Maximized(true)));
    }
}
