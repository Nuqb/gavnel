//! Player control bar, independent of the native video engine.
use super::*;

pub(super) struct State<'a> {
    pub duration: f64,
    pub position: f64,
    pub volume: f64,
    pub muted: bool,
    pub paused: bool,
    pub ended: bool,
    pub fullscreen: bool,
    pub bookmarks: &'a [f64],
}
#[derive(Default)]
pub(super) struct Action {
    pub toggle_play: bool,
    pub audio_changed: bool,
    pub preview: Option<f64>,
    pub seek: Option<f64>,
    pub toggle_fullscreen: bool,
    pub close_requested: bool,
    pub back: bool,
    pub error: Option<String>,
}

pub(super) fn height(bookmarks: &[f64]) -> f32 {
    if bookmarks.is_empty() { 88.0 } else { 128.0 }
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut State<'_>) -> Action {
    let mut action = Action::default();
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
            ui.visuals_mut().override_text_color = Some(Color32::WHITE);
            if !state.bookmarks.is_empty() {
                bookmarks(ui, state, &mut action);
            }
            timeline(ui, state, &mut action);
            transport(ui, state, &mut action);
        });
    action
}

fn timeline(ui: &mut egui::Ui, state: &mut State<'_>, action: &mut Action) {
    ui.add_enabled_ui(state.duration > 0.0, |ui| {
        let response = widgets::playback_slider(
            ui,
            &mut state.position,
            0.0..=state.duration.max(0.01),
            ui.available_width(),
        );
        if state.duration > 0.0 {
            let track = response.rect.shrink2(Vec2::new(7.0, 0.0));
            for seconds in state.bookmarks {
                let x = track.left()
                    + track.width() * (seconds / state.duration).clamp(0.0, 1.0) as f32;
                ui.painter().line_segment(
                    [
                        egui::pos2(x, track.center().y - 5.0),
                        egui::pos2(x, track.center().y + 5.0),
                    ],
                    egui::Stroke::new(2.0, ACCENT),
                );
            }
        }
        if response.changed() {
            action.preview = Some(state.position);
        }
        if response.drag_stopped() || response.changed() && !response.dragged() {
            action.seek = Some(state.position);
        }
        response.on_hover_text(tr("Seek"));
    });
}

fn transport(ui: &mut egui::Ui, state: &mut State<'_>, action: &mut Action) {
    ui.horizontal(|ui| {
        let (glyph, label) = if state.ended {
            (views::Glyph::Refresh, "Replay")
        } else if state.paused {
            (views::Glyph::Play, "Play")
        } else {
            (views::Glyph::Pause, "Pause")
        };
        action.toggle_play = control_button(ui, glyph, tr(label)).clicked();
        ui.push_id("volume-controls", |ui| volume(ui, state, action));
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "{} / {}",
                playback::time(state.position),
                playback::time(state.duration)
            ))
            .size(12.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            action.toggle_fullscreen = ui
                .push_id("fullscreen-controls", |ui| {
                    fullscreen_button(ui, state.fullscreen).clicked()
                })
                .inner;
        });
    });
}

/// Each left-hand action has its own translucent hit target.
fn control_button(ui: &mut egui::Ui, glyph: views::Glyph, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(31.0), egui::Sense::click());
    button_backdrop(ui, rect, &response);
    views::paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        glyph,
        Color32::WHITE,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response.on_hover_text(label)
}

fn button_backdrop(ui: &egui::Ui, rect: egui::Rect, response: &egui::Response) {
    ui.painter()
        .rect_filled(rect, 8.0, Color32::from_black_alpha(95));
    let highlight = ui.ctx().animate_bool_with_time_and_easing(
        response.id.with("hover-highlight"),
        response.enabled()
            && (response.hovered() || response.has_focus() || response.is_pointer_button_down_on()),
        0.1,
        egui::emath::easing::cubic_out,
    );
    let alpha = if response.is_pointer_button_down_on() {
        60.0
    } else {
        36.0
    };
    // Keep a dark rim between the inset highlight and the video behind the button.
    ui.painter().rect_filled(
        rect.shrink(3.0),
        5.0,
        Color32::from_white_alpha((alpha * highlight).round() as u8),
    );
}

fn volume(ui: &mut egui::Ui, state: &mut State<'_>, action: &mut Action) {
    let button = control_button(
        ui,
        if state.muted || state.volume == 0.0 {
            views::Glyph::Muted
        } else {
            views::Glyph::Volume
        },
        tr(if state.muted { "Unmute" } else { "Mute" }),
    );
    if button.clicked() {
        state.muted = !state.muted;
        action.audio_changed = true;
    }
    let id = ui.scope_id().with("volume-expansion");
    let hovered = volume_hovered(ui, &button, id);
    let expansion = ui.ctx().animate_bool_with_time_and_easing(
        id,
        hovered,
        0.12,
        egui::emath::easing::cubic_out,
    );
    if expansion <= 0.0 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(88.0 * expansion, 31.0), egui::Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(rect.min, Vec2::new(88.0, 31.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    // Leave paint space for the thumb outline and hover expansion at either endpoint.
    child.add_space(4.0);
    let slider = widgets::playback_slider(&mut child, &mut state.volume, 0.0..=1.0, 80.0)
        .on_hover_text(tr("Volume"));
    action.audio_changed |= slider.changed();
    let active = slider.dragged() || slider.has_focus();
    ui.ctx()
        .data_mut(|data| data.insert_temp(id, (button.rect.union(rect), active)));
}

fn volume_hovered(ui: &egui::Ui, button: &egui::Response, id: egui::Id) -> bool {
    let previous = ui
        .ctx()
        .data(|data| data.get_temp::<(egui::Rect, bool)>(id));
    button.hovered()
        || button.has_focus()
        || previous.is_some_and(|(rect, active)| {
            active
                || ui.input(|input| {
                    input
                        .pointer
                        .hover_pos()
                        .is_some_and(|pos| rect.contains(pos))
                })
        })
}

fn fullscreen_button(ui: &mut egui::Ui, fullscreen: bool) -> egui::Response {
    let (name, svg, label): (&'static str, &[u8], &str) = if fullscreen {
        (
            "exit-fullscreen",
            include_bytes!("../../assets/icons/exit-fullscreen.svg"),
            "Exit fullscreen",
        )
    } else {
        (
            "fullscreen",
            include_bytes!("../../assets/icons/fullscreen.svg"),
            "Fullscreen",
        )
    };
    let texture = crate::icons::texture(ui.ctx(), name, svg, 32);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(31.0), egui::Sense::click());
    // The right-hand group currently contains only fullscreen; settings can join here.
    button_backdrop(ui, rect, &response);
    ui.painter().image(
        texture.id(),
        rect.shrink(6.0),
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tr(label)));
    response.on_hover_text(tr(label))
}

fn bookmarks(ui: &mut egui::Ui, state: &State<'_>, action: &mut Action) {
    egui::ScrollArea::horizontal()
        .max_height(30.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, seconds) in state.bookmarks.iter().enumerate() {
                    if ui
                        .add_enabled(
                            state.duration > 0.0,
                            egui::Button::new(format!(
                                "{} \u{00b7} {}",
                                index + 1,
                                playback::time(*seconds)
                            )),
                        )
                        .on_hover_text(tr("Jump to this bookmarked moment"))
                        .clicked()
                    {
                        action.seek = Some(*seconds);
                    }
                }
            });
        });
}
