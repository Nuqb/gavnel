use super::*;
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind};
fn ruler_step(duration: f64, width: f32) -> f64 {
    let target = duration / (width / 95.0).max(1.0) as f64;
    [
        0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0,
    ]
    .into_iter()
    .find(|step| *step >= target)
    .unwrap_or(target)
}
impl Editor {
    pub(super) fn receive_filmstrip(
        &mut self,
        context: &egui::Context,
        id: u64,
        result: Result<Vec<egui::ColorImage>, String>,
    ) {
        let Some((_, path)) = &self.strip_pending else {
            return;
        };
        match result {
            Ok(images) => {
                let complete = images.len() == 8;
                let textures = self.filmstrips.entry(path.clone()).or_default();
                for (i, image) in images.into_iter().enumerate().skip(textures.len()) {
                    textures.push(context.load_texture(
                        format!("timeline-{id}-{i}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                if complete {
                    self.strip_pending = None;
                }
            }
            Err(error) => {
                // Retain partial results (or an empty entry) to avoid retrying
                // a broken source on every paint.
                self.filmstrips.entry(path.clone()).or_default();
                self.strip_pending = None;
                self.error = error;
            }
        }
    }

    pub(super) fn request_filmstrip(&mut self, service: &MediaService) {
        let Some(s) = self
            .project
            .as_ref()
            .and_then(|p| p.segments.get(self.selected))
        else {
            return;
        };
        if self.strip_pending.is_some()
            || self.filmstrips.contains_key(&s.info.path)
            || !s.info.path.is_file()
        {
            return;
        }
        self.strip_serial = self.strip_serial.wrapping_add(1);
        if service
            .filmstrip(self.strip_serial, s.info.path.clone())
            .is_ok()
        {
            self.strip_pending = Some((self.strip_serial, s.info.path.clone()));
        }
    }
    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self
            .project
            .as_mut()
            .and_then(|p| p.segments.get_mut(self.selected))
        else {
            return;
        };
        ui.add_space(10.0);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 100.0),
            Sense::click_and_drag(),
        );
        ui.painter()
            .rect_filled(rect, 7.0, Color32::from_rgb(23, 23, 27));
        let lane = Rect::from_min_max(
            rect.min + egui::vec2(9.0, 30.0),
            rect.max - egui::vec2(9.0, 10.0),
        );
        let duration = s.info.duration;
        let x = |t: f64| lane.left() + lane.width() * (t / duration) as f32;
        let step = ruler_step(duration, lane.width());
        let mut tick = 0.0;
        while tick <= duration + step * 0.01 {
            let at = x(tick);
            ui.painter().line_segment(
                [
                    egui::pos2(at, rect.top() + 22.0),
                    egui::pos2(at, rect.top() + 28.0),
                ],
                Stroke::new(1.0, MUTED),
            );
            let label = if duration < 10.0 {
                format!("{tick:.1}s")
            } else {
                playback::time(tick)
            };
            let align = if tick == 0.0 {
                egui::Align2::LEFT_TOP
            } else {
                egui::Align2::CENTER_TOP
            };
            ui.painter().text(
                egui::pos2(at, rect.top() + 5.0),
                align,
                label,
                egui::FontId::monospace(10.0),
                MUTED,
            );
            tick += step;
        }
        ui.painter()
            .rect_filled(lane, 3.0, Color32::from_rgb(42, 44, 50));
        if let Some(frames) = self.filmstrips.get(&s.info.path) {
            let w = lane.width() / 8.0;
            for (i, frame) in frames.iter().enumerate() {
                let tile = Rect::from_min_max(
                    egui::pos2(lane.left() + i as f32 * w, lane.top()),
                    egui::pos2(lane.left() + (i + 1) as f32 * w, lane.bottom()),
                );
                ui.painter().image(
                    frame.id(),
                    tile,
                    Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        } else {
            for i in 1..8 {
                let at = lane.left() + lane.width() * i as f32 / 8.0;
                ui.painter().line_segment(
                    [egui::pos2(at, lane.top()), egui::pos2(at, lane.bottom())],
                    Stroke::new(1.0, LINE),
                );
            }
        }
        let selection = Rect::from_min_max(
            egui::pos2(x(s.start), lane.top()),
            egui::pos2(x(s.end), lane.bottom()),
        );
        for outside in [
            Rect::from_min_max(lane.min, selection.left_bottom()),
            Rect::from_min_max(selection.right_top(), lane.max),
        ] {
            ui.painter()
                .rect_filled(outside, 0.0, Color32::from_black_alpha(185));
        }
        ui.painter()
            .rect_stroke(selection, 3.0, Stroke::new(2.0, ACCENT), StrokeKind::Inside);
        for (time, left) in [(s.start, true), (s.end, false)] {
            let handle = Rect::from_center_size(
                egui::pos2(x(time), lane.center().y),
                Vec2::new(10.0, lane.height()),
            );
            ui.painter().rect_filled(handle, 2.0, ACCENT);
            ui.painter().line_segment(
                [
                    handle.center() - egui::vec2(0.0, 7.0),
                    handle.center() + egui::vec2(0.0, 7.0),
                ],
                Stroke::new(1.5, Color32::from_rgb(55, 32, 15)),
            );
            let drag = ui.interact(
                handle.expand2(egui::vec2(4.0, 0.0)),
                ui.scope_id()
                    .with(if left { "trim-start" } else { "trim-end" }),
                Sense::drag(),
            );
            if drag.dragged()
                && let Some(pos) = drag.interact_pointer_pos()
            {
                let time = ((pos.x - lane.left()) / lane.width()).clamp(0.0, 1.0) as f64 * duration;
                if left {
                    s.start = time.min(s.end - 1.0 / s.info.fps).max(0.0);
                } else {
                    s.end = time.max(s.start + 1.0 / s.info.fps).min(duration);
                }
            }
            drag.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        }
        for (index, (time, label)) in s.info.markers.iter().enumerate() {
            let at = x(*time);
            ui.painter().circle_filled(
                egui::pos2(at, lane.top() + 6.0),
                3.5,
                Color32::from_rgb(108, 195, 255),
            );
            let response = ui.interact(
                Rect::from_center_size(egui::pos2(at, lane.top() + 6.0), Vec2::splat(13.0)),
                ui.scope_id().with(("event", index)),
                Sense::click(),
            );
            if response.clicked() {
                self.cursor = *time;
                self.last_frame = None;
            }
            response.on_hover_text(format!("{} · {}", tr(label), playback::time(*time)));
        }
        let at = x(self.cursor.clamp(0.0, duration));
        ui.painter().line_segment(
            [
                egui::pos2(at, rect.top() + 20.0),
                egui::pos2(at, lane.bottom() + 3.0),
            ],
            Stroke::new(1.5, TEXT),
        );
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(at - 5.0, rect.top() + 18.0),
                egui::pos2(at + 5.0, rect.top() + 18.0),
                egui::pos2(at, rect.top() + 25.0),
            ],
            TEXT,
            Stroke::NONE,
        ));
        if (response.clicked() || response.dragged())
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.cursor = ((pos.x - lane.left()) / lane.width()).clamp(0.0, 1.0) as f64 * duration;
            self.last_frame = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ruler_density_stays_readable() {
        assert_eq!(ruler_step(60.0, 600.0), 10.0);
        assert!(ruler_step(3600.0, 800.0) >= 300.0);
        assert!(ruler_step(1.0, 800.0) < 1.0);
    }
}
