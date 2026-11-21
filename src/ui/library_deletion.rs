use super::*;
use egui::{Align2, Pos2, Rect};
use std::path::Path;

const REFLOW_SECONDS: f64 = 0.24;
const TOAST_SECONDS: f64 = 3.0;

#[derive(Default)]
pub(super) struct Feedback {
    origin: Pos2,
    visible_slots: HashMap<PathBuf, Rect>,
    previous_slots: HashMap<PathBuf, Rect>,
    started: Option<f64>,
    remaining: f32,
}

impl Feedback {
    pub(super) fn begin_frame(&mut self, now: f64) {
        self.visible_slots.clear();
        self.remaining = self
            .started
            .map_or(0.0, |start| remaining_motion(now - start));
        if self.remaining == 0.0 {
            self.previous_slots.clear();
        }
    }

    pub(super) fn set_origin(&mut self, origin: Pos2) {
        self.origin = origin;
    }

    pub(super) fn card_offset(
        &mut self,
        ui: &egui::Ui,
        path: &Path,
        slot: Rect,
        capture: bool,
    ) -> Vec2 {
        let viewport = ui.clip_rect();
        let local = slot.translate(-self.origin.to_vec2());
        let offset = self
            .previous_slots
            .get(path)
            .filter(|from| (from.size() - local.size()).length() < 1.0)
            .map_or(Vec2::ZERO, |from| (from.min - local.min) * self.remaining);
        let painted = slot.translate(offset);
        if !viewport.intersects(painted) && !viewport.intersects(slot) {
            return Vec2::ZERO;
        }
        if capture && viewport.intersects(painted) {
            self.visible_slots
                .insert(path.to_owned(), local.translate(offset));
        }
        if offset != Vec2::ZERO {
            ui.ctx().request_repaint();
        }
        offset
    }

    fn deleted(&mut self, ctx: &egui::Context) {
        std::mem::swap(&mut self.previous_slots, &mut self.visible_slots);
        self.started = Some(ctx.input(|i| i.time));
        ctx.request_repaint();
    }

    fn show_toast(&mut self, ctx: &egui::Context) {
        let Some(start) = self.started else { return };
        let elapsed = ctx.input(|i| i.time) - start;
        if elapsed >= TOAST_SECONDS {
            self.started = None;
            self.previous_slots.clear();
            return;
        }
        let opacity = (elapsed / 0.16)
            .min((TOAST_SECONDS - elapsed) / 0.2)
            .clamp(0.0, 1.0) as f32;
        egui::Area::new(egui::Id::unique("library-delete-toast"))
            .order(egui::Order::Foreground)
            .anchor(
                Align2::RIGHT_BOTTOM,
                Vec2::new(-20.0, -20.0 + 12.0 * (1.0 - opacity)),
            )
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .corner_radius(8.0)
                    .inner_margin(14.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(tr("Clip deleted.")).strong());
                    });
            });
        if opacity < 1.0 {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_secs_f64(
                (TOAST_SECONDS - elapsed - 0.2).max(0.0),
            ));
        }
    }
}

fn remaining_motion(elapsed: f64) -> f32 {
    (1.0 - (elapsed / REFLOW_SECONDS).clamp(0.0, 1.0) as f32).powi(3)
}

impl Gavnel {
    pub(super) fn show_library_deletion(&mut self, ctx: &egui::Context) {
        self.library_deletion.show_toast(ctx);
        let Some(path) = self.delete_pending.clone() else {
            return;
        };
        let clip = self
            .library
            .clips
            .lock()
            .unwrap()
            .iter()
            .find(|clip| clip.path == path)
            .cloned();
        match delete::show(ctx, &path, clip.as_ref(), self.delete_error.as_deref()) {
            delete::Action::None => {}
            delete::Action::Cancel => {
                self.delete_pending = None;
                self.delete_error = None;
            }
            delete::Action::Delete => match self.library.delete(&path) {
                Ok(()) => {
                    self.textures.remove(&path);
                    self.montage_selection.retain(|selected| selected != &path);
                    self.library.refresh(&self.settings.clips_dir);
                    self.library_deletion.deleted(ctx);
                    self.delete_pending = None;
                    self.delete_error = None;
                }
                Err(_) => self.delete_error = Some(tr("Could not delete this clip. It may have moved or be open in another player. Close this dialog and refresh the Library, then try again.").into()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflow_moves_visible_cards_and_settles_at_the_new_slot() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 500.0))),
            time: Some(0.0),
            ..Default::default()
        };
        let _ = super::super::test_render::run(&ctx, input, |ui: &mut egui::Ui| {
            let mut feedback = Feedback::default();
            let visible = Path::new("visible.mp4");
            let hidden = Path::new("hidden.mp4");
            let first = Rect::from_min_size(egui::pos2(20.0, 20.0), Vec2::splat(80.0));
            let old = first.translate(Vec2::new(100.0, 0.0));
            let below = first.translate(Vec2::new(0.0, 600.0));
            feedback.begin_frame(0.0);
            feedback.card_offset(ui, visible, old, true);
            feedback.card_offset(ui, hidden, below, true);
            feedback.deleted(ui.ctx());

            feedback.begin_frame(0.0);
            assert_eq!(feedback.card_offset(ui, visible, first, false).x, 100.0);
            assert_eq!(feedback.card_offset(ui, hidden, first, false), Vec2::ZERO);
            feedback.begin_frame(REFLOW_SECONDS / 2.0);
            assert_eq!(feedback.card_offset(ui, visible, first, false).x, 12.5);
            feedback.begin_frame(REFLOW_SECONDS);
            assert_eq!(feedback.card_offset(ui, visible, first, false), Vec2::ZERO);
        });
    }
}
