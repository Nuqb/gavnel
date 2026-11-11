use super::*;
use crate::config::AudioCaptureMode;
impl Gavnel {
    pub(super) fn audio_capture_controls(&mut self, ui: &mut egui::Ui) {
        let modes = [
            (AudioCaptureMode::AllPc, "All PC audio"),
            (
                AudioCaptureMode::SelectedDevices,
                "Selected playback devices",
            ),
            (AudioCaptureMode::GameOnly, "Game only"),
            (AudioCaptureMode::SelectedApps, "Selected apps"),
        ];
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Capture audio")).strong());
            let current = self.settings.audio_mode();
            super::motion::ComboBox::from_id_salt("audio-capture-mode")
                .selected_text(tr(modes.iter().find(|(m, _)| *m == current).unwrap().1))
                .show_ui(ui, |ui| {
                    for (mode, label) in modes {
                        let enabled = !matches!(
                            mode,
                            AudioCaptureMode::GameOnly | AudioCaptureMode::SelectedApps
                        ) || crate::platform::application_audio_supported();
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::selectable(mode == current, tr(label)),
                            )
                            .on_disabled_hover_text(tr("Requires Windows 11"))
                            .clicked()
                        {
                            self.settings.audio_capture_mode = Some(mode);
                        }
                    }
                });
        });
        if self.settings.audio_mode() == AudioCaptureMode::GameOnly
            && self.settings.mode == CaptureMode::Monitor
        {
            ui.colored_label(ACCENT, tr("Choose a game or window for game-only audio."));
        }
        if self.settings.audio_mode() != AudioCaptureMode::SelectedApps {
            return;
        }
        let key = egui::Id::unique("audio-process-list-cache");
        let running = ui.ctx().data_mut(|data| {
            let cached = data.get_temp::<(Instant, Vec<crate::audio::process::AppProcess>)>(key);
            if let Some((time, apps)) = cached
                && time.elapsed() < Duration::from_secs(2)
            {
                return apps;
            }
            let apps = crate::audio::process::processes();
            data.insert_temp(key, (Instant::now(), apps.clone()));
            apps
        });
        let mut names: Vec<_> = running
            .iter()
            .filter(|a| a.pid > 4 && !a.executable.eq_ignore_ascii_case("gavnel.exe"))
            .map(|a| a.executable.clone())
            .collect();
        names.extend(self.settings.audio_apps.clone());
        names.sort_by_key(|n| n.to_ascii_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        egui::ScrollArea::vertical()
            .id_salt("audio-app-list")
            .max_height(150.0)
            .show_rows(ui, 36.0, names.len(), |ui, range| {
                for name in &names[range] {
                    let mut selected = self
                        .settings
                        .audio_apps
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(name));
                    let available = running
                        .iter()
                        .any(|a| a.executable.eq_ignore_ascii_case(name));
                    ui.horizontal(|ui| {
                        if let Some(app) = running
                            .iter()
                            .find(|a| a.executable.eq_ignore_ascii_case(name))
                            && let Some(icon) = crate::app_icons::texture(ui.ctx(), app.pid)
                        {
                            ui.add(egui::Image::new(&icon).fit_to_exact_size(Vec2::splat(22.0)));
                        }

                        let can_select = selected || self.settings.audio_apps.len() < 8;
                        let response = ui
                            .add_enabled_ui(can_select, |ui| {
                                super::checkbox::checkbox(ui, &mut selected, name)
                            })
                            .inner;
                        let changed = response.changed();
                        if !can_select {
                            response.on_hover_text(tr("Select up to eight applications"));
                        }
                        if changed {
                            self.settings
                                .audio_apps
                                .retain(|n| !n.eq_ignore_ascii_case(name));
                            if selected && self.settings.audio_apps.len() < 8 {
                                self.settings.audio_apps.push(name.clone());
                            }
                        }
                        if !available {
                            ui.label(tr("Not running"));
                        }
                    });
                }
            });
    }
}
