//! Shared dropdown motion. Native egui popups retain ownership of input/focus;
//! a paint-only snapshot provides the closing animation after native dismissal.
use super::*;
use egui::{
    Id, LayerId, Order, Rect, Vec2,
    epaint::{ClippedShape, Shape},
    layers::ShapeIdx,
};
use std::sync::Arc;

const OPEN_SECONDS: f64 = 0.18;
const CLOSE_SECONDS: f64 = 0.12;

#[derive(Clone, Default)]
struct Motion {
    from: f32,
    target: bool,
    started: f64,
    duration: f64,
}
impl Motion {
    fn value(&self, now: f64) -> f32 {
        if self.duration <= 0.0 {
            return f32::from(self.target);
        }
        let t = ((now - self.started) / self.duration).clamp(0.0, 1.0) as f32;
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (f32::from(self.target) - self.from) * eased
    }
    fn target(&mut self, target: bool, now: f64) {
        if self.target != target {
            self.from = self.value(now);
            self.target = target;
            self.started = now;
            self.duration = if target { OPEN_SECONDS } else { CLOSE_SECONDS };
        }
    }
    fn moving(&self, now: f64) -> bool {
        (self.value(now) - f32::from(self.target)).abs() > 0.0001
    }
}

#[derive(Clone)]
struct PaintSnapshot {
    shapes: Arc<Vec<ClippedShape>>,
    rect: Rect,
    anchor: Rect,
}
#[derive(Clone, Default)]
struct PopupMotion {
    motion: Motion,
    snapshot: Option<PaintSnapshot>,
    seen: u64,
}
fn state_id(id: Id) -> Id {
    id.with("gavnel-dropdown-motion")
}
fn registry_id() -> Id {
    Id::new("gavnel-animated-popups")
}

fn reveal(rect: Rect, anchor: Rect, amount: f32) -> Rect {
    let mut clip = rect.expand(8.0);
    if rect.center().y < anchor.center().y {
        clip.min.y = clip.max.y - clip.height() * amount;
    } else {
        clip.max.y = clip.min.y + clip.height() * amount;
    }
    clip
}

/// Restrict input as well as painting to the revealed part of the menu.
pub(super) fn clip_contents(ui: &mut egui::Ui, id: Id) {
    let state = ui.ctx().data(|d| d.get_temp::<PopupMotion>(state_id(id)));
    if let Some(state) = state
        && let Some(snapshot) = state.snapshot
    {
        let amount = state.motion.value(ui.input(|i| i.time));
        if amount < 1.0 {
            ui.set_clip_rect(ui.clip_rect().intersect(reveal(
                snapshot.rect,
                snapshot.anchor,
                amount,
            )));
        }
    }
}

fn paint(ctx: &egui::Context, id: Id, snapshot: &PaintSnapshot, amount: f32) {
    if amount <= 0.0001 {
        return;
    }
    let offset = Vec2::ZERO;
    let clip = reveal(snapshot.rect, snapshot.anchor, amount).translate(offset);
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, id));
    for entry in snapshot.shapes.iter() {
        let mut shape = entry.shape.clone();
        shape.translate(offset);
        egui::epaint::shape_transform::adjust_colors(&mut shape, move |color| {
            if *color != Color32::PLACEHOLDER {
                *color = color.gamma_multiply(amount);
            }
        });
        let original_clip = entry.clip_rect.translate(offset);
        painter
            .with_clip_rect(if amount < 1.0 {
                original_clip.intersect(clip)
            } else {
                original_clip
            })
            .add(shape);
    }
}

pub(super) fn popup(ctx: &egui::Context, id: Id, anchor: Rect, open: bool, rendered: bool) {
    let now = ctx.input(|i| i.time);
    let mut state = ctx
        .data_mut(|d| d.get_temp::<PopupMotion>(state_id(id)))
        .unwrap_or_default();
    state.seen = ctx.cumulative_pass_nr();
    state.motion.target(open, now);
    if rendered {
        let layer = LayerId::new(Order::Foreground, id);
        let mut raw: Vec<_> = ctx.graphics(|g| {
            g.get(layer)
                .map(|p| p.all_entries().cloned().collect())
                .unwrap_or_default()
        });
        if raw.iter().any(|s| !matches!(s.shape, Shape::Noop))
            && let Some(area) = egui::containers::AreaState::load(ctx, id)
        {
            // Replace egui's built-in opening fade with our reversible curve;
            // otherwise reopening during collapse would flash back to transparent.
            if let Some(visible_at) = area.last_became_visible_at {
                let duration = ctx.style().animation_time as f64;
                let age = now - visible_at + ctx.input(|i| i.predicted_dt) as f64 / 2.0;
                let native = if duration > 0.0 {
                    egui::emath::easing::quadratic_out((age / duration).clamp(0.0, 1.0) as f32)
                } else {
                    1.0
                };
                if native > 0.0 && native < 1.0 {
                    for entry in &mut raw {
                        egui::epaint::shape_transform::adjust_colors(
                            &mut entry.shape,
                            move |color| {
                                if *color != Color32::PLACEHOLDER {
                                    *color = color.gamma_multiply(1.0 / native);
                                }
                            },
                        );
                    }
                }
            }
            state.snapshot = Some(PaintSnapshot {
                shapes: Arc::new(raw),
                rect: area.rect(),
                anchor,
            });
        }
        ctx.graphics_mut(|g| {
            if let Some(list) = g.get_mut(layer) {
                for i in 0..list.next_idx().0 {
                    list.reset_shape(ShapeIdx(i));
                }
            }
        });
    }
    let amount = state.motion.value(now);
    if let Some(snapshot) = &state.snapshot {
        paint(ctx, id, snapshot, amount);
    }
    if state.motion.moving(now) {
        ctx.request_repaint_after(Duration::from_millis(16));
    } else if !open {
        state.snapshot = None;
    }
    ctx.data_mut(|d| {
        let mut active = d.get_temp::<Vec<Id>>(registry_id()).unwrap_or_default();
        active.retain(|value| *value != id);
        if state.snapshot.is_some() {
            active.push(id);
        }
        d.insert_temp(registry_id(), active);
        d.insert_temp(state_id(id), state);
    });
}

/// Finish a disappearing control's menu too (page changes, a closed parent, etc.).
pub(super) fn finish(ctx: &egui::Context) {
    let active = ctx
        .data_mut(|d| d.get_temp::<Vec<Id>>(registry_id()))
        .unwrap_or_default();
    for id in active {
        let state = ctx.data_mut(|d| d.get_temp::<PopupMotion>(state_id(id)));
        if let Some(state) = state
            && state.seen != ctx.cumulative_pass_nr()
            && let Some(snapshot) = state.snapshot
        {
            egui::Popup::close_id(ctx, id);
            popup(ctx, id, snapshot.anchor, false, false);
        }
    }
}

#[cfg(test)]
#[path = "motion_tests.rs"]
mod tests;

pub(super) fn chevron(ui: &egui::Ui, id: Id, rect: Rect, open: bool, color: Color32) {
    let amount = ui.ctx().animate_bool_with_time_and_easing(
        id.with("chevron"),
        open,
        0.16,
        egui::emath::easing::cubic_out,
    );
    let rotation = egui::emath::Rot2::from_angle(-std::f32::consts::PI * amount);
    let points = [
        Vec2::new(-4.0, -2.0),
        Vec2::new(0.0, 2.0),
        Vec2::new(4.0, -2.0),
    ];
    ui.painter().add(Shape::line(
        points
            .into_iter()
            .map(|p| rect.center() + rotation * p)
            .collect(),
        egui::Stroke::new(1.6_f32, color),
    ));
}

/// Builder compatible with the subset used by Gavnel; all selectors share motion.
pub(super) struct ComboBox {
    inner: egui::ComboBox,
    salt: Id,
}
impl ComboBox {
    pub fn from_id_salt(salt: impl std::hash::Hash) -> Self {
        Self {
            inner: egui::ComboBox::from_id_salt(&salt),
            salt: Id::new(&salt),
        }
    }
    pub fn width(mut self, width: f32) -> Self {
        self.inner = self.inner.width(width);
        self
    }
    pub fn selected_text(mut self, text: impl Into<egui::WidgetText>) -> Self {
        self.inner = self.inner.selected_text(text);
        self
    }
    pub fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let id = ui.make_persistent_id(self.salt).with("popup");
        let result = self
            .inner
            .icon(move |ui, rect, visuals, open| {
                chevron(ui, id, rect, open, visuals.fg_stroke.color)
            })
            .show_ui(ui, |ui| {
                clip_contents(ui, id);
                content(ui)
            });
        popup(
            ui.ctx(),
            id,
            result.response.rect,
            egui::Popup::is_id_open(ui.ctx(), id),
            result.inner.is_some(),
        );
        result
    }
    #[cfg(test)]
    pub fn is_open(ctx: &egui::Context, id: Id) -> bool {
        egui::ComboBox::is_open(ctx, id)
    }
}

pub(super) fn vertical_menu_button<R>(
    ui: &mut egui::Ui,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(24.0, 24.0), egui::Sense::click());
    let color = if response.hovered() || response.has_focus() {
        ACCENT
    } else {
        TEXT
    };
    for offset in [-6.0, 0.0, 6.0] {
        ui.painter()
            .circle_filled(rect.center() + Vec2::new(0.0, offset), 1.6, color);
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Clip actions"));
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let inner = egui::Popup::menu(&response)
        .show(|ui| {
            clip_contents(ui, ui.layer_id().id);
            content(ui)
        })
        .map(|result| result.inner);
    let id = egui::Popup::default_response_id(&response);
    popup(
        ui.ctx(),
        id,
        response.rect,
        egui::Popup::is_id_open(ui.ctx(), id),
        inner.is_some(),
    );
    egui::InnerResponse { response, inner }
}
