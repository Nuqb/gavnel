use super::*;
use egui::{Pos2, Rect, Sense, Stroke, StrokeKind};
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Handle {
    Move,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
#[derive(Clone, Copy)]
pub(super) struct Drag {
    handle: Handle,
    rect: [f64; 4],
    origin: Pos2,
}
fn hit(rect: Rect, pos: Pos2) -> Option<Handle> {
    if !rect.expand(9.0).contains(pos) {
        return None;
    }
    let l = (pos.x - rect.left()).abs() < 9.0;
    let r = (pos.x - rect.right()).abs() < 9.0;
    let t = (pos.y - rect.top()).abs() < 9.0;
    let b = (pos.y - rect.bottom()).abs() < 9.0;
    match (l, r, t, b) {
        (true, _, true, _) => Some(Handle::TopLeft),
        (_, true, true, _) => Some(Handle::TopRight),
        (true, _, _, true) => Some(Handle::BottomLeft),
        (_, true, _, true) => Some(Handle::BottomRight),
        (true, _, _, _) => Some(Handle::Left),
        (_, true, _, _) => Some(Handle::Right),
        (_, _, true, _) => Some(Handle::Top),
        (_, _, _, true) => Some(Handle::Bottom),
        _ if rect.contains(pos) => Some(Handle::Move),
        _ => None,
    }
}
fn resize(rect: [f64; 4], delta: [f64; 2], handle: Handle, minimum: [f64; 2]) -> [f64; 4] {
    let [x, y, w, h] = rect;
    let [dx, dy] = delta;
    if handle == Handle::Move {
        return [
            (x + dx).clamp(0.0, 1.0 - w),
            (y + dy).clamp(0.0, 1.0 - h),
            w,
            h,
        ];
    }
    let (mut l, mut r, mut t, mut b) = (x, x + w, y, y + h);
    if matches!(handle, Handle::Left | Handle::TopLeft | Handle::BottomLeft) {
        l = (l + dx).clamp(0.0, r - minimum[0]);
    }
    if matches!(
        handle,
        Handle::Right | Handle::TopRight | Handle::BottomRight
    ) {
        r = (r + dx).clamp(l + minimum[0], 1.0);
    }
    if matches!(handle, Handle::Top | Handle::TopLeft | Handle::TopRight) {
        t = (t + dy).clamp(0.0, b - minimum[1]);
    }
    if matches!(
        handle,
        Handle::Bottom | Handle::BottomLeft | Handle::BottomRight
    ) {
        b = (b + dy).clamp(t + minimum[1], 1.0);
    }
    [l, t, r - l, b - t]
}
impl Editor {
    pub(super) fn preview_canvas(
        &mut self,
        ui: &mut egui::Ui,
        player: &mut Option<crate::player::Player>,
    ) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
        ui.painter().rect_filled(rect, 8.0, Color32::BLACK);
        let editing_crop = self.tab == Tab::Crop && player.as_ref().is_none_or(|p| p.paused());
        if !editing_crop
            && !self.export_open
            && let Some(p) = player
        {
            let crop = if self.rendered_playback {
                None
            } else {
                self.project
                    .as_ref()
                    .and_then(|p| p.segments.get(self.selected))
                    .map(|s| s.crop.normalized(&s.info))
            };
            let dimensions = if self.rendered_playback {
                self.project.as_ref().map(|project| {
                    let (w, h, _) = crate::media::preview_dimensions(project);
                    (w as f32, h as f32)
                })
            } else {
                self.project
                    .as_ref()
                    .and_then(|project| project.segments.get(self.selected))
                    .map(|segment| {
                        let [_, _, w, h] = crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
                        (
                            segment.info.width as f32 * w as f32,
                            segment.info.height as f32 * h as f32,
                        )
                    })
            };
            let video_rect = dimensions
                .map(|(w, h)| fit_video(rect, w, h))
                .unwrap_or(rect);
            if let Err(e) = p.layout_cropped(video_rect, ui.ctx().pixels_per_point(), true, crop) {
                self.error = e.to_string();
            }
            return;
        }
        if let Some(p) = player {
            let _ = p.layout(rect, ui.ctx().pixels_per_point(), false);
        }
        let Some(texture) = &self.texture else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                tr("Loading preview…"),
                egui::FontId::proportional(14.0),
                MUTED,
            );
            return;
        };
        let size = texture.resolution_vec2();
        let factor = (rect.width() / size.x).min(rect.height() / size.y);
        let image = Rect::from_center_size(rect.center(), size * factor);
        ui.painter().image(
            texture.id(),
            image,
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        if editing_crop {
            self.crop_overlay(ui, image);
        } else {
            let button_rect = Rect::from_center_size(rect.center(), Vec2::splat(54.0));
            ui.painter()
                .circle_filled(rect.center(), 27.0, Color32::from_black_alpha(180));
            crate::icons::paint(
                ui.painter(),
                "editor-preview-play",
                include_bytes!("../../../assets/icons/play.svg"),
                button_rect.shrink(15.0),
                TEXT,
            );
            if response.clicked() {
                self.play_requested = true;
            }
        }
    }
    fn crop_overlay(&mut self, ui: &mut egui::Ui, image: Rect) {
        let Some(s) = self
            .project
            .as_mut()
            .and_then(|p| p.segments.get_mut(self.selected))
        else {
            return;
        };
        let [x, y, w, h] = s.crop.normalized(&s.info);
        let crop = Rect::from_min_size(
            image.min + egui::vec2(x as f32 * image.width(), y as f32 * image.height()),
            egui::vec2(w as f32 * image.width(), h as f32 * image.height()),
        );
        let response = ui.interact(
            image.expand(8.0),
            ui.scope_id().with("crop-frame"),
            Sense::drag(),
        );
        if response.drag_started()
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            && let Some(handle) = hit(crop, origin)
        {
            self.crop_drag = Some(Drag {
                handle,
                rect: [x, y, w, h],
                origin,
            });
        }
        if response.dragged()
            && let Some(drag) = self.crop_drag
            && let Some(pos) = response.interact_pointer_pos()
        {
            let delta = [
                (pos.x - drag.origin.x) as f64 / image.width() as f64,
                (pos.y - drag.origin.y) as f64 / image.height() as f64,
            ];
            let minimum = [
                (16.0 / s.info.width as f64).min(drag.rect[2]),
                (16.0 / s.info.height as f64).min(drag.rect[3]),
            ];
            let rect = resize(drag.rect, delta, drag.handle, minimum);
            if drag.handle == Handle::Move && s.crop.aspect != crate::media::Aspect::Custom {
                s.crop.x = if rect[2] < 1.0 {
                    rect[0] / (1.0 - rect[2])
                } else {
                    0.5
                };
                s.crop.y = if rect[3] < 1.0 {
                    rect[1] / (1.0 - rect[3])
                } else {
                    0.5
                };
            } else {
                s.crop.set_rect(rect);
            }
        }
        if response.drag_stopped() {
            self.crop_drag = None;
        }
        for shade in [
            Rect::from_min_max(image.min, egui::pos2(crop.left(), image.bottom())),
            Rect::from_min_max(egui::pos2(crop.right(), image.top()), image.max),
            Rect::from_min_max(egui::pos2(crop.left(), image.top()), crop.right_top()),
            Rect::from_min_max(crop.left_bottom(), egui::pos2(crop.right(), image.bottom())),
        ] {
            ui.painter()
                .rect_filled(shade, 0.0, Color32::from_black_alpha(155));
        }
        ui.painter()
            .rect_stroke(crop, 0.0, Stroke::new(1.5, TEXT), StrokeKind::Inside);
        for fraction in [1.0 / 3.0, 2.0 / 3.0] {
            let x = crop.left() + crop.width() * fraction;
            let y = crop.top() + crop.height() * fraction;
            ui.painter().line_segment(
                [egui::pos2(x, crop.top()), egui::pos2(x, crop.bottom())],
                Stroke::new(1.0, Color32::from_white_alpha(55)),
            );
            ui.painter().line_segment(
                [egui::pos2(crop.left(), y), egui::pos2(crop.right(), y)],
                Stroke::new(1.0, Color32::from_white_alpha(55)),
            );
        }
        for pos in [
            crop.left_top(),
            crop.right_top(),
            crop.left_bottom(),
            crop.right_bottom(),
            crop.center_top(),
            crop.center_bottom(),
            crop.left_center(),
            crop.right_center(),
        ] {
            let r = Rect::from_center_size(pos, Vec2::splat(7.0));
            ui.painter().rect_filled(r, 1.0, TEXT);
            ui.painter().rect_stroke(
                r,
                1.0,
                Stroke::new(1.0, Color32::BLACK),
                StrokeKind::Outside,
            );
        }
        let handle = self
            .crop_drag
            .map(|d| d.handle)
            .or_else(|| response.hover_pos().and_then(|p| hit(crop, p)));
        if let Some(handle) = handle {
            response.on_hover_cursor(match handle {
                Handle::Move => egui::CursorIcon::Grab,
                Handle::Left | Handle::Right => egui::CursorIcon::ResizeHorizontal,
                Handle::Top | Handle::Bottom => egui::CursorIcon::ResizeVertical,
                Handle::TopLeft | Handle::BottomRight => egui::CursorIcon::ResizeNwSe,
                Handle::TopRight | Handle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_edges_and_corners_resize_and_stay_in_bounds() {
        let original = [0.0, 0.0, 1.0, 1.0];
        assert_eq!(
            resize(original, [-0.25, -0.5], Handle::BottomRight, [0.01, 0.01]),
            [0.0, 0.0, 0.75, 0.5]
        );
        let r = resize(
            [0.2, 0.2, 0.5, 0.5],
            [2.0, -2.0],
            Handle::Move,
            [0.01, 0.01],
        );
        assert_eq!(r, [0.5, 0.0, 0.5, 0.5]);
        let r = resize(original, [2.0, 2.0], Handle::TopLeft, [0.1, 0.1]);
        assert!(r[2] >= 0.099 && r[3] >= 0.099);
    }
    #[test]
    fn every_border_has_a_resize_target() {
        let r = Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(110.0, 110.0));
        assert_eq!(hit(r, r.right_center()), Some(Handle::Right));
        assert_eq!(hit(r, r.left_top()), Some(Handle::TopLeft));
        assert_eq!(hit(r, r.center()), Some(Handle::Move));
    }
}

fn fit_video(rect: Rect, width: f32, height: f32) -> Rect {
    if width <= 0.0 || height <= 0.0 {
        return rect;
    }
    let scale = (rect.width() / width).min(rect.height() / height);
    Rect::from_center_size(rect.center(), Vec2::new(width * scale, height * scale))
}

#[cfg(test)]
mod preview_layout_tests {
    use super::*;
    #[test]
    fn video_fits_and_centers_in_wide_and_tall_canvases() {
        for rect in [
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 400.0)),
            Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 900.0)),
        ] {
            let fit = fit_video(rect, 1920.0, 1080.0);
            assert_eq!(fit.center(), rect.center());
            assert!(fit.width() <= rect.width() && fit.height() <= rect.height());
            assert!((fit.width() / fit.height() - 16.0 / 9.0).abs() < 0.001);
        }
    }
}
