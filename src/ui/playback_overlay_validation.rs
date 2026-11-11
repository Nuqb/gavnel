//! Isolated native rendering diagnostic; no recordings, settings, or input devices.
use super::*;
use anyhow::{Result, ensure};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Dwm::{DWMWA_BORDER_COLOR, DwmGetWindowAttribute},
    Graphics::Gdi::ClientToScreen,
    UI::WindowsAndMessaging::{
        GWL_STYLE, GWLP_HWNDPARENT, GetWindowLongPtrW, GetWindowRect, IsChild, IsWindowVisible,
        SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, WS_CHILD, WS_POPUP,
    },
};

struct Validation {
    parent: HWND,
    frames: usize,
    result: Arc<Mutex<Option<String>>>,
}
impl eframe::App for Validation {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(30, 75, 100)))
            .show(root, |ui| {
                let size = ui.available_size();
                let rect = egui::Rect::from_min_size(
                    egui::pos2(0.0, size.y - 88.0),
                    Vec2::new(size.x, 88.0),
                );
                let mut state = playback_controls::State {
                    duration: 120.0,
                    position: 30.0,
                    volume: 1.0,
                    muted: false,
                    paused: true,
                    ended: false,
                    fullscreen: false,
                    bookmarks: &[],
                };
                let visible = !matches!(self.frames, 2 | 3);
                let action = show(ui.ctx(), self.parent, rect, &mut state, visible);
                if let Some(error) = action.error {
                    *self.result.lock().unwrap() = Some(error);
                }
                if self.frames >= 2 {
                    match native::window(self.parent) {
                        Some(window)
                            if unsafe { GetWindowLongPtrW(window, GWLP_HWNDPARENT) }
                                == self.parent.0 as isize =>
                        {
                            if let Err(error) = verify_no_border(window) {
                                *self.result.lock().unwrap() = Some(error.to_string());
                            }
                            if unsafe { IsWindowVisible(window).as_bool() } != visible {
                                *self.result.lock().unwrap() =
                                    Some(format!("Player controls did not show or hide in place on frame {} (expected {visible}, parent visible {}, style {:x}).", self.frames, unsafe { IsWindowVisible(self.parent).as_bool() }, unsafe { GetWindowLongPtrW(window, GWL_STYLE) }));
                            }
                            let expected =
                                native::screen_rect(self.parent, rect, ui.ctx().pixels_per_point());
                            if let Err(error) = verify_position(window, expected) {
                                *self.result.lock().unwrap() = Some(error.to_string());
                            }
                            if self.frames == 4
                                && let Err(error) = verify_attached_motion(self.parent, window)
                            {
                                *self.result.lock().unwrap() = Some(error.to_string());
                            }
                        }
                        _ => {
                            *self.result.lock().unwrap() = Some(
                                "Native control overlay was not attached to its player.".into(),
                            )
                        }
                    }
                }
            });
        self.frames += 1;
        if self.frames >= 8 {
            root.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        root.ctx().request_repaint_after(Duration::from_millis(30));
    }
}

/// Move the parent without another layout/attach call: the timeline must follow immediately.
fn verify_attached_motion(parent: HWND, window: HWND) -> Result<()> {
    unsafe {
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        ensure!(
            IsChild(parent, window).as_bool() && style & WS_CHILD.0 != 0 && style & WS_POPUP.0 == 0,
            "Player controls are still an independently animated top-level window."
        );
        let mut before = RECT::default();
        let mut owner = RECT::default();
        GetWindowRect(window, &mut before)?;
        GetWindowRect(parent, &mut owner)?;
        let flags = SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE;
        SetWindowPos(parent, None, owner.left + 37, owner.top + 29, 0, 0, flags)?;
        let expected = egui::Rect::from_min_size(
            egui::pos2((before.left + 37) as f32, (before.top + 29) as f32),
            egui::vec2(
                (before.right - before.left) as f32,
                (before.bottom - before.top) as f32,
            ),
        );
        let result = verify_position(window, expected);
        SetWindowPos(parent, None, owner.left, owner.top, 0, 0, flags)?;
        result
    }
}

fn verify_position(window: HWND, expected: egui::Rect) -> Result<()> {
    let mut actual = RECT::default();
    unsafe {
        GetWindowRect(window, &mut actual)?;
    }
    ensure!(
        actual.left == expected.left().round() as i32
            && actual.top == expected.top().round() as i32
            && actual.right - actual.left == expected.width().round() as i32
            && actual.bottom - actual.top == expected.height().round() as i32,
        "Player controls moved away from the video: actual ({}, {}, {}, {}), expected {:?}.",
        actual.left,
        actual.top,
        actual.right,
        actual.bottom,
        expected
    );
    Ok(())
}

pub(crate) fn run() -> Result<()> {
    let result = Arc::new(Mutex::new(None));
    let shared = result.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Gavnel overlay validation")
            .with_inner_size([640.0, 360.0])
            .with_position([-10000.0, -10000.0])
            .with_active(false)
            // Match the opaque main app; only the control viewport is transparent.
            .with_transparent(false),
        ..Default::default()
    };
    eframe::run_native(
        "Gavnel overlay validation",
        options,
        Box::new(move |cc| {
            use eframe::glow::HasContext;
            if let Some(gl) = &cc.gl
                && unsafe {
                    gl.get_framebuffer_attachment_parameter_i32(
                        eframe::glow::DRAW_FRAMEBUFFER,
                        eframe::glow::BACK_LEFT,
                        eframe::glow::FRAMEBUFFER_ATTACHMENT_ALPHA_SIZE,
                    )
                } < 8
            {
                return Err(
                    "The OpenGL surface does not support transparent player controls.".into(),
                );
            }
            super::super::theme::apply(&cc.egui_ctx);
            let parent = match cc.window_handle()?.as_raw() {
                RawWindowHandle::Win32(handle) => HWND(handle.hwnd.get() as _),
                _ => return Err("Expected a Windows player window.".into()),
            };
            Ok(Box::new(Validation {
                parent,
                frames: 0,
                result: shared,
            }))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let error = result.lock().unwrap().take();
    ensure!(error.is_none(), "{}", error.unwrap_or_default());
    Ok(())
}

fn verify_no_border(window: HWND) -> Result<()> {
    let mut outer = RECT::default();
    let mut client_origin = POINT::default();
    unsafe {
        GetWindowRect(window, &mut outer)?;
        ClientToScreen(window, &mut client_origin).ok()?;
    }
    ensure!(
        client_origin.x == outer.left && client_origin.y == outer.top,
        "Player overlay has a non-client edge: client starts at ({}, {}), window at ({}, {}).",
        client_origin.x,
        client_origin.y,
        outer.left,
        outer.top
    );

    let mut color = 0_u32;
    let supported = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_BORDER_COLOR,
            std::ptr::from_mut(&mut color).cast(),
            std::mem::size_of_val(&color) as u32,
        )
    };
    if supported.is_ok() {
        ensure!(
            color == 0xffff_fffe,
            "Player overlay still has a native border (color 0x{color:08X})."
        );
    }
    Ok(())
}
