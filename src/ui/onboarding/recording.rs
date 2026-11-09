use super::*;
use crate::capture_hotkeys::CaptureAction;

impl Wizard {
    pub(super) fn quality_page(&mut self, ui: &mut egui::Ui) {
        let presets = [
            (tr("Small files"), "720p · 30 FPS · 8 Mbps", 720, 30, 8),
            (tr("Smooth HD"), "720p · 60 FPS · 15 Mbps", 720, 60, 15),
            (tr("Balanced"), "1080p · 60 FPS · 20 Mbps", 1080, 60, 20),
            (tr("Extra detail"), "1440p · 60 FPS · 30 Mbps", 1440, 60, 30),
        ];
        for row in presets.chunks(2) {
            ui.columns(2, |columns| {
                for (ui, &(title, caption, height, fps, bitrate)) in columns.iter_mut().zip(row) {
                    let selected = (self.draft.height, self.draft.fps, self.draft.bitrate_mbps)
                        == (height, fps, bitrate);
                    if ui
                        .add_sized(
                            [ui.available_width(), 84.0],
                            egui::Button::new(
                                RichText::new(format!("{}\n{caption}", tr(title))).size(14.0),
                            )
                            .fill(if selected { ACCENT_DARK } else { CARD })
                            .stroke(Stroke::new(1.0_f32, if selected { ACCENT } else { LINE })),
                        )
                        .clicked()
                    {
                        self.draft.height = height;
                        self.draft.fps = fps;
                        self.draft.bitrate_mbps = bitrate;
                    }
                }
            });
            ui.add_space(10.0);
        }
        ui.collapsing(tr("Customize quality"), |ui| {
            super::super::widgets::dropdown(
                ui,
                "setup-resolution",
                &mut self.draft.height,
                &[(720, "720p"), (1080, "1080p"), (1440, "1440p")],
            );
            super::super::widgets::dropdown(
                ui,
                "setup-fps",
                &mut self.draft.fps,
                &[(30, "30 FPS"), (60, "60 FPS"), (120, "120 FPS")],
            );
            super::super::widgets::slider(ui, &mut self.draft.bitrate_mbps, 5..=50, "Mbps");
        });
    }

    pub(super) fn hotkey_page(&mut self, ui: &mut egui::Ui) {
        for action in CaptureAction::ALL {
            let bindings = self.draft.capture_bindings();
            let Some(binding) = bindings
                .iter()
                .find(|binding| binding.action == action && !binding.key.is_empty())
                .or_else(|| bindings.iter().find(|binding| binding.action == action))
            else {
                continue;
            };
            ui.horizontal(|ui| {
                super::super::shortcuts::action_icon(ui, action);
                ui.label(RichText::new(tr(action.label())).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if self.listening == Some(binding.id) {
                        tr("Press a key…")
                    } else if binding.key.is_empty() {
                        tr("Not set")
                    } else {
                        &binding.key
                    };
                    if ui
                        .add_sized(
                            [180.0, 40.0],
                            egui::Button::new(RichText::new(label).color(ACCENT)),
                        )
                        .clicked()
                    {
                        self.listening = Some(binding.id);
                        self.error.clear();
                    }
                });
            });
            ui.add_space(9.0);
        }
        ui.label(
            RichText::new(tr("Click a shortcut to change it. Escape cancels."))
                .size(12.0)
                .color(MUTED),
        );
        ui.add_space(14.0);
        ui.label(RichText::new(tr("Replay length")).strong());
        let previous_duration = self.draft.replay_seconds;
        super::super::widgets::dropdown(
            ui,
            "setup-replay",
            &mut self.draft.replay_seconds,
            &[
                (15, tr("15 seconds")),
                (30, tr("30 seconds")),
                (60, tr("60 seconds")),
                (120, tr("2 minutes")),
            ],
        );
        if self.draft.replay_seconds != previous_duration {
            self.set_replay_seconds(self.draft.replay_seconds);
        }
    }

    pub(super) fn set_replay_seconds(&mut self, seconds: u32) {
        let bindings = self.draft.edit_capture_bindings();
        let id = bindings
            .iter()
            .find(|binding| binding.action == CaptureAction::Clip && !binding.key.is_empty())
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| binding.action == CaptureAction::Clip)
            })
            .map(|binding| binding.id);
        if let Some(binding) = bindings.iter_mut().find(|binding| Some(binding.id) == id) {
            binding.clip_seconds = seconds;
        }
        self.draft.replay_seconds = seconds;
        self.draft.sync_primary_bindings();
    }

    pub(super) fn capture_keys(&mut self, ctx: &egui::Context) {
        let Some(id) = self.listening else {
            return;
        };
        for event in ctx.input(|input| input.events.clone()) {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            self.listening = None;
            if key == egui::Key::Escape {
                return;
            }
            let mut parts = Vec::new();
            if modifiers.ctrl {
                parts.push("Control".to_owned());
            }
            if modifiers.alt {
                parts.push("Alt".to_owned());
            }
            if modifiers.shift {
                parts.push("Shift".to_owned());
            }
            parts.push(key.name().to_owned());
            let mut candidate = self.draft.clone();
            if let Some(binding) = candidate
                .edit_capture_bindings()
                .iter_mut()
                .find(|binding| binding.id == id)
            {
                binding.key = parts.join("+");
            }
            candidate.sync_primary_bindings();
            match super::super::shortcuts::parse_bindings(&candidate) {
                Ok(_) => {
                    self.draft = candidate;
                    self.error.clear();
                }
                Err(error) => self.error = error.to_string(),
            }
            return;
        }
    }
}
