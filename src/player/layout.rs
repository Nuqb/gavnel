//! Keep the native video child inside the image bounds; egui paints the black bars.
use super::Player;
use anyhow::Result;
use eframe::egui;
use windows::Win32::{
    Foundation::RECT,
    Graphics::Gdi::{CombineRgn, CreateRectRgn, DeleteObject, RGN_DIFF, SetWindowRgn},
    Media::MediaFoundation::{MFARGB, MFVideoNormalizedRect},
    UI::WindowsAndMessaging::{
        IsWindowVisible, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
        ShowWindow,
    },
};

impl Player {
    pub(crate) fn image_rect(&self) -> Option<egui::Rect> {
        let rect = self.window_rect.get();
        rect.is_positive().then_some(rect)
    }

    pub fn layout(&self, rect: egui::Rect, pixels: f32, visible: bool) -> Result<()> {
        self.layout_cropped(rect, pixels, visible, None)
    }

    pub fn layout_cropped(
        &self,
        rect: egui::Rect,
        pixels: f32,
        visible: bool,
        crop: Option<[f64; 4]>,
    ) -> Result<()> {
        if self.duration() > 0.0
            && let Some(time) = self.pending_seek.take()
        {
            self.seek(time)?;
        }
        if !visible {
            unsafe {
                let _ = ShowWindow(self.window.0, SW_HIDE);
            }
            return Ok(());
        }
        let bounds = fitted_bounds(rect, pixels, self.video_size(crop));
        // Size and repaint the renderer while hidden, before exposing its swapchain.
        unsafe {
            SetWindowPos(
                self.window.0,
                None,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )?;
        }
        self.window_rect.set(egui::Rect::from_min_max(
            egui::pos2(bounds.left as f32, bounds.top as f32) / pixels,
            egui::pos2(bounds.right as f32, bounds.bottom as f32) / pixels,
        ));
        if self.duration() > 0.0 {
            self.update_video(bounds, crop)?;
        }
        unsafe {
            if self.ready() && !IsWindowVisible(self.window.0).as_bool() {
                let _ = ShowWindow(self.window.0, SW_SHOWNOACTIVATE);
            }
        }
        Ok(())
    }

    fn video_size(&self, crop: Option<[f64; 4]>) -> [f64; 2] {
        let (mut width, mut height) = (0, 0);
        unsafe {
            let _ = self
                .engine
                .GetNativeVideoSize(Some(&mut width), Some(&mut height));
        }
        let [_, _, crop_w, crop_h] = crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
        [width as f64 * crop_w, height as f64 * crop_h]
    }

    fn update_video(&self, bounds: RECT, crop: Option<[f64; 4]>) -> Result<()> {
        let source = crop.map(|[x, y, w, h]| MFVideoNormalizedRect {
            left: x as f32,
            top: y as f32,
            right: (x + w) as f32,
            bottom: (y + h) as f32,
        });
        // The renderer fills its entire child window. Letterboxing belongs to
        // the opaque parent, never to unpainted space around a native swapchain.
        let destination = RECT {
            left: 0,
            top: 0,
            right: bounds.right - bounds.left,
            bottom: bounds.bottom - bounds.top,
        };
        unsafe {
            self.engine.UpdateVideoStream(
                source.as_ref().map(|r| r as *const _),
                Some(&destination),
                Some(&MFARGB {
                    rgbAlpha: 255,
                    rgbRed: 0,
                    rgbGreen: 0,
                    rgbBlue: 0,
                }),
            )?;
        }
        Ok(())
    }

    pub fn set_occlusion(&self, _surface: egui::Rect, pixels: f32, popup: Option<egui::Rect>) {
        // Popups are in parent coordinates; subtract the fitted child's origin,
        // which differs from the surface origin when there are black bars.
        let rect = self.window_rect.get();
        unsafe {
            let Some(overlap) = popup.map(|r| r.intersect(rect)).filter(|r| r.is_positive()) else {
                SetWindowRgn(self.window.0, None, true);
                return;
            };
            let full = CreateRectRgn(
                0,
                0,
                (rect.width() * pixels).round() as i32,
                (rect.height() * pixels).round() as i32,
            );
            let cut = CreateRectRgn(
                ((overlap.left() - rect.left()) * pixels).floor() as i32,
                ((overlap.top() - rect.top()) * pixels).floor() as i32,
                ((overlap.right() - rect.left()) * pixels).ceil() as i32,
                ((overlap.bottom() - rect.top()) * pixels).ceil() as i32,
            );
            CombineRgn(Some(full), Some(full), Some(cut), RGN_DIFF);
            let _ = DeleteObject(cut.into());
            if SetWindowRgn(self.window.0, Some(full), true) == 0 {
                let _ = DeleteObject(full.into());
            }
        }
    }
}

fn fitted_bounds(surface: egui::Rect, pixels: f32, [width, height]: [f64; 2]) -> RECT {
    let outer_w = (surface.width() * pixels).round().max(1.0) as i32;
    let outer_h = (surface.height() * pixels).round().max(1.0) as i32;
    let (w, h) = if width > 0.0 && height > 0.0 {
        let scale = (outer_w as f64 / width).min(outer_h as f64 / height);
        (
            (width * scale).round().clamp(1.0, outer_w as f64) as i32,
            (height * scale).round().clamp(1.0, outer_h as f64) as i32,
        )
    } else {
        (outer_w, outer_h)
    };
    let left = (surface.left() * pixels).round() as i32 + (outer_w - w) / 2;
    let top = (surface.top() * pixels).round() as i32 + (outer_h - h) / 2;
    RECT {
        left,
        top,
        right: left + w,
        bottom: top + h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_surface_has_equal_pillarboxes() {
        let surface = egui::Rect::from_min_size(egui::pos2(94.0, 180.0), egui::vec2(1800.0, 824.0));
        let bounds = fitted_bounds(surface, 1.0, [1920.0, 1080.0]);
        assert_eq!(
            (bounds.left, bounds.top, bounds.right, bounds.bottom),
            (261, 180, 1726, 1004)
        );
    }

    #[test]
    fn fit_stays_centered_and_inside_surface_at_different_dpi_and_ratios() {
        let surface = egui::Rect::from_min_size(egui::pos2(94.0, 180.0), egui::vec2(901.0, 603.0));
        for pixels in [1.0, 1.25, 1.5, 2.0] {
            for size in [
                [1920.0, 1080.0],
                [1080.0, 1920.0],
                [3840.0, 1080.0],
                [400.0, 400.0],
            ] {
                let bounds = fitted_bounds(surface, pixels, size);
                let outer = fitted_bounds(surface, pixels, [0.0, 0.0]);
                assert!(bounds.left >= outer.left && bounds.top >= outer.top);
                assert!(bounds.right <= outer.right && bounds.bottom <= outer.bottom);
                assert!(((bounds.left - outer.left) - (outer.right - bounds.right)).abs() <= 1);
                assert!(((bounds.top - outer.top) - (outer.bottom - bounds.bottom)).abs() <= 1);
                let w = (bounds.right - bounds.left) as f64;
                let h = (bounds.bottom - bounds.top) as f64;
                assert!((w - h * size[0] / size[1]).abs() <= size[0] / size[1] + 1.0);
            }
        }
    }
}
