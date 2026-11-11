//! A transparent viewport overlays controls without resizing or copying video frames.
use super::*;
use crate::player::overlay as native;

#[derive(Default)]
pub(super) struct Visibility {
    last_pointer: Option<egui::Pos2>,
    last_activity: Option<f64>,
}
impl Visibility {
    pub fn visible(
        &mut self,
        now: f64,
        pointer: Option<egui::Pos2>,
        controls: egui::Rect,
        paused: bool,
    ) -> bool {
        let last = self.last_activity.get_or_insert(now);
        if pointer.is_some()
            && (pointer != self.last_pointer
                || pointer.is_some_and(|point| controls.contains(point)))
        {
            *last = now;
        }
        self.last_pointer = pointer;
        paused || now - *last < 2.5
    }
}

pub(super) fn show(
    ctx: &egui::Context,
    parent: windows::Win32::Foundation::HWND,
    controls: egui::Rect,
    state: &mut playback_controls::State<'_>,
    visible: bool,
) -> playback_controls::Action {
    let pixels = ctx.pixels_per_point();
    let screen = native::screen_rect(parent, controls, pixels);
    let native_scale = ctx
        .input(|input| input.viewport().native_pixels_per_point)
        .unwrap_or(pixels);
    let builder = egui::ViewportBuilder::default()
        .with_title(native::TITLE)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_active(false)
        .with_transparent(true)
        // The native window is attached and painted before it is ever shown.
        .with_visible(false)
        // Winit reapplies this after native attachment, so use child coordinates.
        .with_position(controls.min * pixels / native_scale)
        .with_inner_size(screen.size() / native_scale);
    let mut action = ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("playback-overlay"),
        builder,
        |root, class| {
            if class == egui::ViewportClass::EmbeddedWindow {
                return playback_controls::show(root, state);
            }
            if let Err(error) = native::prepare(parent, screen) {
                return playback_controls::Action {
                    error: Some(error.to_string()),
                    ..Default::default()
                };
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| {
                    let mut action = playback_controls::show(ui, state);
                    let keys = ui.input_mut(|input| {
                        (
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::F)
                                | input.consume_key(egui::Modifiers::NONE, egui::Key::F11),
                        )
                    });
                    action.toggle_play |= keys.0;
                    action.toggle_fullscreen |= keys.2 || keys.1 && state.fullscreen;
                    action.back = keys.1 && !state.fullscreen;
                    if ui.input(|input| input.viewport().close_requested()) {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::CancelClose);
                        action.close_requested = true;
                    }
                    action
                })
                .inner
        },
    );
    if let Err(error) = native::attach(parent, screen, visible) {
        action.error = Some(error.to_string());
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_hide_during_playback_and_reappear_on_motion_or_pause() {
        let mut visibility = Visibility::default();
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 200.0), egui::pos2(800.0, 300.0));
        assert!(visibility.visible(0.0, Some(egui::pos2(50.0, 50.0)), rect, false));
        assert!(!visibility.visible(3.0, Some(egui::pos2(50.0, 50.0)), rect, false));
        assert!(visibility.visible(3.1, Some(egui::pos2(51.0, 50.0)), rect, false));
        assert!(visibility.visible(9.0, None, rect, true));
        assert!(visibility.visible(10.0, Some(egui::pos2(100.0, 250.0)), rect, false));
        assert!(visibility.visible(20.0, Some(egui::pos2(100.0, 250.0)), rect, false));
    }
}

#[cfg(debug_assertions)]
#[path = "playback_overlay_validation.rs"]
mod validation;
#[cfg(debug_assertions)]
pub(crate) use validation::run as validate_native;
