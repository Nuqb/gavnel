use super::views::section;
use super::*;
use crate::update::State;
use std::path::Path;

impl Gavnel {
    pub(super) fn update_notice(&mut self, ui: &mut egui::Ui) {
        let version = match &self.updates.state {
            State::Available(release) => Some(release.version.to_string()),
            State::Ready(prepared) => Some(prepared.release.version.to_string()),
            _ => None,
        };
        if let Some(version) = version {
            if ui
                .button(i18n::message(
                    "Gavnel {version} is available",
                    &[("version", &version)],
                ))
                .clicked()
            {
                self.page = Page::Settings;
                self.settings_page = SettingsPage::General;
            }
            ui.add_space(8.0);
        }
    }
    pub(super) fn update_settings(&mut self, ui: &mut egui::Ui) {
        self.updates.poll(ui.ctx(), false);
        section(ui, tr("Updates"), |ui| {
            ui.label(i18n::message(
                "Installed version: {version}",
                &[("version", env!("CARGO_PKG_VERSION"))],
            ));
            ui.add_space(8.0);
            self.update_controls(ui);
        });
    }
    fn update_controls(&mut self, ui: &mut egui::Ui) {
        match &self.updates.state {
            State::Checking => {
                ui.spinner();
                ui.label(tr("Checking for updates..."));
            }
            State::Current => {
                ui.label(tr("You have the latest version."));
            }
            State::Available(release) => {
                ui.label(i18n::message(
                    "Gavnel {version} is available",
                    &[("version", &release.version.to_string())],
                ));
                let release = release.clone();
                if ui.link(tr("Release notes")).clicked() {
                    let _ = platform::open(Path::new(&release.url));
                }
                if ui
                    .add_enabled(
                        self.updates.installed,
                        egui::Button::new(tr("Download update")),
                    )
                    .clicked()
                {
                    self.updates.download(release, ui.ctx());
                }
            }
            State::Downloading { received, total } => {
                ui.add(
                    egui::ProgressBar::new(received.load(Ordering::Relaxed) as f32 / *total as f32)
                        .show_percentage(),
                );
                ui.label(tr("Downloading and verifying update..."));
            }
            State::Ready(_) => {
                ui.label(tr("Update verified. Restart when you are ready; pending recordings will finish saving."));
                if ui.button(tr("Restart to update")).clicked() {
                    match self
                        .apply_preferences(false)
                        .and_then(|()| self.updates.restart())
                    {
                        Ok(()) => self.window_actions.exit(),
                        Err(error) => self.message = format!("{error:#}"),
                    }
                }
            }
            State::Failed(error) => {
                ui.colored_label(DANGER, error);
            }
            State::Idle => {}
        }
        if !matches!(
            self.updates.state,
            State::Checking | State::Downloading { .. } | State::Ready(_)
        ) && ui.button(tr("Check for updates")).clicked()
        {
            self.updates.check(ui.ctx());
        }
    }
}
