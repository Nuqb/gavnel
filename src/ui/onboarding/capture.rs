use super::super::views::{Glyph, paint_icon};
use super::*;

impl Wizard {
    pub(super) fn capture_page(
        &mut self,
        ui: &mut egui::Ui,
        devices: &Devices<'_>,
        action: &mut Option<Action>,
    ) {
        ui.columns(2, |columns| {
            for (column, (monitor, title, glyph)) in columns.iter_mut().zip([
                (false, tr("Game / window"), Glyph::Game),
                (true, tr("Whole screen"), Glyph::Grid),
            ]) {
                let selected = (self.draft.mode == CaptureMode::Monitor) == monitor;
                let response = egui::Frame::new()
                    .fill(if selected { ACCENT_DARK } else { CARD })
                    .stroke(Stroke::new(1.0_f32, if selected { ACCENT } else { LINE }))
                    .corner_radius(10.0)
                    .inner_margin(16.0)
                    .show(column, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(65.0);
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        paint_icon(
                            ui.painter(),
                            rect,
                            glyph,
                            if selected { ACCENT } else { MUTED },
                        );
                        ui.add_space(8.0);
                        ui.label(RichText::new(title).strong().size(16.0));
                    })
                    .response;
                if column
                    .interact(response.rect, column.scope_id().with(title), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.select_source(if monitor {
                        CaptureMode::Monitor
                    } else {
                        CaptureMode::Automatic
                    });
                }
            }
        });
        ui.add_space(24.0);
        if self.draft.mode == CaptureMode::Monitor {
            super::super::motion::ComboBox::from_id_salt("setup-monitor")
                .width(400.0)
                .selected_text(
                    devices
                        .monitors
                        .iter()
                        .find(|m| m.index == self.draft.monitor_index)
                        .map_or(tr("Choose a screen"), |m| m.label.as_str()),
                )
                .show_ui(ui, |ui| {
                    for monitor in devices.monitors {
                        ui.selectable_value(
                            &mut self.draft.monitor_index,
                            monitor.index,
                            &monitor.label,
                        );
                    }
                });
        } else {
            let selected = if self.draft.mode == CaptureMode::Automatic {
                tr("Automatically detect games")
            } else if self.draft.window_title.is_empty() {
                tr("Choose a window")
            } else {
                &self.draft.window_title
            };
            super::super::motion::ComboBox::from_id_salt("setup-window")
                .width(400.0)
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            self.draft.mode == CaptureMode::Automatic,
                            tr("Automatically detect games"),
                        )
                        .clicked()
                    {
                        self.select_source(CaptureMode::Automatic);
                    }
                    for window in devices.windows {
                        if ui
                            .selectable_label(
                                self.draft.mode == CaptureMode::Window
                                    && self.draft.window_title == window.title,
                                &window.title,
                            )
                            .clicked()
                        {
                            self.select_source(CaptureMode::Window);
                            self.draft.window_title = window.title.clone();
                            self.draft.window_executable = window.executable.clone();
                        }
                    }
                });
            if self.draft.mode == CaptureMode::Automatic {
                ui.add_space(12.0);
                let mut sessions = self.draft.recording_mode.automatic();
                if ui
                    .checkbox(&mut sessions, tr("Record full game sessions"))
                    .changed()
                {
                    self.draft.recording_mode = if sessions {
                        RecordingMode::AutomaticSessions
                    } else {
                        RecordingMode::Clips
                    };
                }
            }
        }
        ui.add_space(12.0);
        if ui.small_button(tr("Refresh sources")).clicked() {
            *action = Some(Action::Refresh);
        }
    }

    fn select_source(&mut self, mode: CaptureMode) {
        self.draft.mode = mode;
        if self.draft.recording_mode == RecordingMode::Off
            || (mode != CaptureMode::Automatic && self.draft.recording_mode.automatic())
        {
            self.draft.recording_mode = RecordingMode::Clips;
        }
    }
}
