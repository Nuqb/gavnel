use super::*;
use egui::{Align2, FontId, Rect, Sense, Stroke, Vec2};

/// Destructive controls use their own red palette in every interaction state.
pub(super) fn destructive_button(
    ui: &mut egui::Ui,
    label: &str,
    prominent: bool,
) -> egui::Response {
    ui.scope(|ui| {
        let visuals = ui.visuals_mut();
        let text = if prominent {
            Color32::from_rgb(255, 222, 225)
        } else {
            DANGER
        };
        visuals.override_text_color = Some(text);
        visuals.selection.bg_fill = DANGER_DARK;
        visuals.selection.stroke = Stroke::new(1.0_f32, DANGER);
        for (state, fill, border) in [
            (
                &mut visuals.widgets.inactive,
                if prominent {
                    Color32::from_rgb(108, 33, 43)
                } else {
                    Color32::TRANSPARENT
                },
                if prominent {
                    Color32::from_rgb(166, 57, 70)
                } else {
                    Color32::TRANSPARENT
                },
            ),
            (
                &mut visuals.widgets.hovered,
                if prominent {
                    Color32::from_rgb(145, 41, 53)
                } else {
                    DANGER_DARK
                },
                DANGER,
            ),
            (
                &mut visuals.widgets.active,
                if prominent {
                    Color32::from_rgb(88, 25, 34)
                } else {
                    Color32::from_rgb(78, 30, 38)
                },
                DANGER,
            ),
            (&mut visuals.widgets.open, DANGER_DARK, DANGER),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0_f32, border);
            state.fg_stroke = Stroke::new(1.0_f32, text);
        }
        let button =
            egui::Button::new(RichText::new(label).strong().color(text)).corner_radius(7.0);
        if prominent {
            ui.add_sized([132.0, 40.0], button)
        } else {
            ui.add(button)
        }
    })
    .inner
}

pub(super) fn clickable_cursor(ctx: &egui::Context) {
    // Preserve text editing, resizing, and dragging cursors supplied by widgets.
    if ctx.output(|output| output.cursor_icon) != egui::CursorIcon::Default {
        return;
    }
    let hovered = ctx.interaction_snapshot(|interaction| interaction.hovered.clone());
    if hovered.into_iter().any(|id| {
        ctx.read_response(id).is_some_and(|response| {
            response.enabled() && response.hovered() && response.sense.senses_click()
        })
    }) {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

pub(super) fn recording_modes(ui: &mut egui::Ui, mode: &mut RecordingMode) -> bool {
    let previous = *mode;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.spacing_mut().interact_size.y = 22.0;
        ui.spacing_mut().button_padding.y = 3.0;
        for (value, title, detail) in recording_mode_options() {
            ui.radio_value(mode, value, RichText::new(title).strong());
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(RichText::new(detail).size(12.0).color(MUTED));
            });
            ui.add_space(8.0);
        }
    });
    *mode != previous
}

fn recording_mode_options() -> [(RecordingMode, &'static str, &'static str); 3] {
    [
        (
            RecordingMode::Clips,
            tr("Manual capture"),
            tr("Use shortcuts for clips, bookmarks, and starting or stopping a long recording."),
        ),
        (
            RecordingMode::AutomaticSessions,
            tr("Automatically record entire game session"),
            tr(
                "Gavnel records when a supported game opens and saves when it closes. Clip hotkeys still work.",
            ),
        ),
        (
            RecordingMode::Off,
            tr("Recording off"),
            tr("Turns off capture. No clips or full recordings will be captured."),
        ),
    ]
}

fn recording_mode_help_content(ui: &mut egui::Ui) {
    ui.set_max_width(340.0);
    ui.label(RichText::new(tr("Recording modes")).size(16.0).strong());
    for (_, title, description) in recording_mode_options() {
        ui.add_space(10.0);
        ui.label(RichText::new(title).strong());
        ui.add(egui::Label::new(RichText::new(description).size(12.0).color(MUTED)).wrap());
    }
}

pub(super) fn recording_mode_help(ui: &mut egui::Ui) {
    let response = ui.add(
        egui::Button::new(RichText::new("?").strong().color(MUTED))
            .corner_radius(12.0)
            .min_size(Vec2::splat(24.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            tr("About recording modes"),
        )
    });
    response.clone().on_hover_ui(recording_mode_help_content);
    let id = egui::Popup::default_response_id(&response);
    let shown = egui::Popup::menu(&response).show(|ui| {
        super::motion::clip_contents(ui, id);
        recording_mode_help_content(ui);
    });
    super::motion::popup(
        ui.ctx(),
        id,
        response.rect,
        egui::Popup::is_id_open(ui.ctx(), id),
        shown.is_some(),
    );
}

pub(super) fn dropdown<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    options: &[(T, &str)],
) {
    let selected = options
        .iter()
        .find(|(option, _)| *option == *value)
        .map(|(_, title)| *title)
        .unwrap_or(tr("Custom"));
    super::motion::ComboBox::from_id_salt(id)
        .width(ui.available_width().min(380.0))
        .selected_text(tr(selected))
        .show_ui(ui, |ui| {
            for (option, title) in options {
                ui.selectable_value(value, *option, tr(title));
            }
        });
}

pub(super) fn quick_panel(
    ui: &egui::Ui,
    anchor: &egui::Response,
    id: egui::Id,
    open: &mut bool,
    opened_from_toolbar: bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    let child_was_open = egui::Popup::is_any_open(ui.ctx());
    let was_open = *open;
    // A ComboBox owns egui's single memory-backed popup slot. The settings
    // panel therefore keeps its own open state, independent of child menus.
    let shown = egui::Popup::from_response(anchor)
        .id(id)
        .open_bool(open)
        .width(364.0)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .frame(
            egui::Frame::popup(ui.style())
                .fill(Color32::from_rgb(31, 31, 34))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(59, 59, 64)))
                .corner_radius(11.0)
                .inner_margin(16.0),
        )
        .show(|ui| {
            super::motion::clip_contents(ui, id);
            ui.set_width(332.0);
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
            ui.spacing_mut().button_padding = Vec2::new(12.0, 8.0);
            ui.spacing_mut().interact_size.y = 34.0;
            content(ui);
        });
    let rendered = shown.is_some();
    if let Some(shown) = shown
        && *open
        && !opened_from_toolbar
        && !anchor.clicked()
        && !child_was_open
        && !egui::Popup::is_any_open(ui.ctx())
        && ui.input(|input| input.pointer.any_click())
        && shown.response.clicked_elsewhere()
    {
        *open = false;
    }
    if !*open && (was_open || anchor.clicked()) {
        egui::Popup::close_all(ui.ctx());
    }
    super::motion::popup(ui.ctx(), id, anchor.rect, *open, rendered);
}
pub(super) fn playback_slider<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    width: f32,
) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 20.0;
        ui.spacing_mut().slider_width = width;
        let visuals = ui.visuals_mut();
        visuals.handle_shape = egui::style::HandleShape::Circle;
        visuals.selection.bg_fill = ACCENT;
        for state in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
        ] {
            state.bg_fill = Color32::from_rgb(55, 55, 62);
            state.fg_stroke = Stroke::new(2.0_f32, TEXT);
        }
        ui.add(egui::Slider::new(value, range).show_value(false))
    })
    .inner
}

pub(super) fn slider<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: std::ops::RangeInclusive<T>,
    unit: &str,
) -> egui::Response {
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 85.0).clamp(120.0, 260.0);
        let response = playback_slider(ui, value, range.clone(), width);
        let scale = if unit == "%" { 100.0 } else { 1.0 };
        let mut number = value.to_f64() * scale;
        let number_response = ui.add(
            egui::DragValue::new(&mut number)
                .range(range.start().to_f64() * scale..=range.end().to_f64() * scale)
                .speed(if unit == "%" { 1.0 } else { 0.5 })
                .max_decimals(0)
                .suffix(if unit == "%" {
                    "%".to_owned()
                } else {
                    format!(" {unit}")
                })
                .update_while_editing(false),
        );
        if number_response.changed() {
            *value = T::from_f64(number / scale);
        }
        response.union(number_response)
    })
    .inner
}

pub(super) fn toggle(ui: &mut egui::Ui, value: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(38.0, 22.0), Sense::click());
    if response.clicked() || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Space)))
    {
        *value = !*value;
        response.mark_changed();
    }
    let background = if *value {
        ACCENT
    } else {
        Color32::from_rgb(68, 68, 73)
    };
    ui.painter().rect_filled(rect, 11.0, background);
    if response.hovered() {
        ui.painter().rect_stroke(
            rect,
            11.0,
            Stroke::new(1.0_f32, TEXT),
            egui::StrokeKind::Inside,
        );
    }
    let x = if *value {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    ui.painter().circle_filled(
        egui::pos2(x, rect.center().y),
        8.0,
        Color32::from_rgb(250, 250, 252),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub(super) fn field(ui: &mut egui::Ui, text: &mut String, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 38.0],
        egui::TextEdit::singleline(text)
            .font(FontId::proportional(14.0))
            .margin(egui::Margin::symmetric(12, 9))
            .desired_width(width),
    )
}
pub(super) fn segments<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    let width = ui.available_width();
    egui::Frame::new()
        .fill(Color32::from_rgb(32, 32, 36))
        .corner_radius(8.0)
        .inner_margin(4.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal(|ui| {
                let width = (width - 8.0 - 4.0 * (options.len() - 1) as f32) / options.len() as f32;
                for (option, label) in options {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
                    let selected = *value == *option;
                    if selected || response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            6.0,
                            if response.hovered() {
                                Color32::from_rgb(75, 67, 58)
                            } else if selected {
                                Color32::from_rgb(62, 62, 67)
                            } else {
                                Color32::from_rgb(43, 43, 48)
                            },
                        );
                    }
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        *label,
                        FontId::proportional(13.0),
                        if selected { TEXT } else { MUTED },
                    );
                    if response.clicked() {
                        *value = *option;
                        changed = true;
                    }
                }
            });
        });
    changed
}
pub(super) fn arrow(painter: &egui::Painter, rect: Rect, down: bool, color: Color32) {
    let center = rect.center();
    let offsets = if down {
        [
            Vec2::new(-4.0, -2.0),
            Vec2::new(0.0, 2.0),
            Vec2::new(4.0, -2.0),
        ]
    } else {
        [
            Vec2::new(-2.0, -4.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(-2.0, 4.0),
        ]
    };
    painter.add(egui::Shape::line(
        offsets.into_iter().map(|p| center + p).collect(),
        Stroke::new(1.5_f32, color),
    ));
}

#[cfg(test)]
mod popup_tests {
    use super::*;
    struct PanelTest {
        open: bool,
        combo: Rect,
        option: Rect,
        combo_id: egui::Id,
        selected: u32,
    }
    fn frame(ctx: &egui::Context, state: &mut PanelTest, events: Vec<egui::Event>) {
        let _ = crate::ui::test_render::run(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let anchor = ui.button(tr("Recorder"));
                    quick_panel(
                        ui,
                        &anchor,
                        egui::Id::unique("test-settings"),
                        &mut state.open,
                        false,
                        |ui| {
                            let response = super::motion::ComboBox::from_id_salt("microphone-test")
                                .selected_text(tr("Off"))
                                .show_ui(ui, |ui| {
                                    state.option =
                                        ui.selectable_value(&mut state.selected, 0, tr("Off")).rect;
                                    ui.selectable_value(
                                        &mut state.selected,
                                        1,
                                        tr("Windows default"),
                                    );
                                });
                            state.combo = response.response.rect;
                            state.combo_id = response.response.id;
                        },
                    );
                });
            },
        );
    }
    fn click(ctx: &egui::Context, state: &mut PanelTest, position: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                ctx,
                state,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    #[test]
    fn microphone_menu_opens_and_selects_without_closing_settings() {
        let ctx = egui::Context::default();
        let mut state = PanelTest {
            open: true,
            combo: Rect::NOTHING,
            option: Rect::NOTHING,
            combo_id: egui::Id::NULL,
            selected: 0,
        };
        for _ in 0..16 {
            frame(&ctx, &mut state, vec![]);
        }
        let position = state.combo.center();
        click(&ctx, &mut state, position);
        for _ in 0..16 {
            frame(&ctx, &mut state, vec![]);
        }
        assert!(
            state.open,
            "Opening the microphone list closed the parent panel"
        );
        assert!(
            super::motion::ComboBox::is_open(&ctx, state.combo_id),
            "The microphone device list did not stay open"
        );
        let position = state.option.center();
        click(&ctx, &mut state, position);
        frame(&ctx, &mut state, vec![]);
        assert!(
            state.open,
            "Choosing a microphone option closed the settings panel"
        );
        assert!(!super::motion::ComboBox::is_open(&ctx, state.combo_id));
        click(&ctx, &mut state, egui::pos2(780.0, 580.0));
        assert!(!state.open, "Clicking outside should dismiss the panel");
    }
}

#[cfg(test)]
mod recording_mode_tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        mode: &mut RecordingMode,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        crate::ui::test_render::run(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(900.0, 420.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading(tr("Recording Mode"));
                    recording_modes(ui, mode);
                });
            },
        )
    }
    fn find_text(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find_text(shape, label)),
            _ => None,
        }
    }
    #[test]
    fn three_recording_choices_are_selectable_without_a_desktop_window() {
        let ctx = egui::Context::default();
        let mut mode = RecordingMode::Clips;
        for (title, expected) in [
            (
                tr("Automatically record entire game session"),
                RecordingMode::AutomaticSessions,
            ),
            (tr("Recording off"), RecordingMode::Off),
            (tr("Manual capture"), RecordingMode::Clips),
        ] {
            let selected_title = recording_mode_options()
                .into_iter()
                .find(|(value, _, _)| *value == mode)
                .unwrap()
                .1;
            let mut output = frame(&ctx, &mut mode, vec![]);
            for _ in 0..2 {
                output = frame(&ctx, &mut mode, vec![]);
            }
            let anchor = output
                .shapes
                .iter()
                .find_map(|shape| find_text(&shape.shape, selected_title))
                .expect("Selected mode missing from dropdown");
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut mode,
                    vec![
                        egui::Event::PointerMoved(anchor),
                        egui::Event::PointerButton {
                            pos: anchor,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            for _ in 0..16 {
                output = frame(&ctx, &mut mode, vec![]);
            }
            let position = output
                .shapes
                .iter()
                .find_map(|shape| find_text(&shape.shape, title))
                .expect("Recording option missing from the rendered settings");
            assert!(
                position.y < 420.0,
                "Recording option is below the initial settings view"
            );
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut mode,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            assert_eq!(mode, expected);
        }
    }
}

#[cfg(test)]
mod editable_slider_tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        value: &mut f32,
        unit: &str,
        events: Vec<egui::Event>,
    ) -> egui::Rect {
        let mut rect = egui::Rect::NOTHING;
        let _ = crate::ui::test_render::run(
            ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600.0, 200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = slider(
                        ui,
                        value,
                        if unit == "%" {
                            0.0..=2.0
                        } else {
                            -100.0..=-25.0
                        },
                        unit,
                    )
                    .rect;
                });
            },
        );
        rect
    }
    #[test]
    fn clicking_value_accepts_typed_percentage_and_negative_db() {
        for (unit, initial, typed, expected) in
            [("%", 1.0, "63", 0.63), ("dB", -75.0, "-55", -55.0)]
        {
            let ctx = egui::Context::default();
            let mut value = initial;
            frame(&ctx, &mut value, unit, vec![]);
            let rect = frame(&ctx, &mut value, unit, vec![]);
            let pos = egui::pos2(rect.right() - 12.0, rect.center().y);
            frame(
                &ctx,
                &mut value,
                unit,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            );
            frame(
                &ctx,
                &mut value,
                unit,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            frame(&ctx, &mut value, unit, vec![]);
            frame(
                &ctx,
                &mut value,
                unit,
                vec![egui::Event::Text(typed.into())],
            );
            frame(
                &ctx,
                &mut value,
                unit,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
            assert!((value - expected).abs() < 0.001, "{unit}: {value}");
        }
    }
}

#[cfg(test)]
mod hover_feedback_tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        kind: u8,
        pos: Option<egui::Pos2>,
    ) -> (egui::Rect, egui::FullOutput) {
        let mut rect = egui::Rect::NOTHING;
        let output = crate::ui::test_render::run(
            ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(400.0, 200.0),
                )),
                events: pos
                    .map(|pos| vec![egui::Event::PointerMoved(pos)])
                    .unwrap_or_default(),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    rect = match kind {
                        0 => crate::ui::clips_menu::manage_hotkeys_button(ui).rect,
                        1 => ui.add_enabled(false, egui::Button::new("Disabled")).rect,
                        2 => ui.text_edit_singleline(&mut "Text".to_owned()).rect,
                        _ => {
                            ui.allocate_exact_size(Vec2::new(120.0, 40.0), Sense::click())
                                .0
                        }
                    };
                });
            },
        );
        (rect, output)
    }
    #[test]
    fn manage_hotkeys_highlights_and_clickable_controls_use_hand_cursor() {
        for kind in [0, 3] {
            let ctx = egui::Context::default();
            crate::ui::theme::apply(&ctx);
            frame(&ctx, kind, None);
            let (rect, _) = frame(&ctx, kind, None);
            let (_, hovered) = frame(&ctx, kind, Some(rect.center()));
            assert_eq!(
                hovered.platform_output.cursor_icon,
                egui::CursorIcon::PointingHand
            );
            if kind == 0 {
                assert!(hovered.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Rect(rect) if rect.fill == Color32::from_rgb(48,44,41))), "Manage Hotkeys must paint its hover highlight");
            }
        }
    }
    #[test]
    fn disabled_controls_and_text_fields_keep_their_appropriate_cursors() {
        for (kind, expected) in [(1, egui::CursorIcon::Default), (2, egui::CursorIcon::Text)] {
            let ctx = egui::Context::default();
            crate::ui::theme::apply(&ctx);
            frame(&ctx, kind, None);
            let (rect, _) = frame(&ctx, kind, None);
            let (_, output) = frame(&ctx, kind, Some(rect.center()));
            assert_eq!(output.platform_output.cursor_icon, expected);
        }
    }
}
