//! An isolated test window: no recorder, tray, hotkey registration, or saved writes.
use super::*;

struct Preview {
    wizard: Wizard,
    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    inputs: Vec<AudioDevice>,
    finished: bool,
}
impl Preview {
    fn new() -> Self {
        let mut wizard = Wizard::new(Settings::load_preserving_storage().0, false);
        wizard.preview = true;
        Self {
            wizard,
            monitors: sources::monitors(),
            windows: sources::windows(),
            inputs: audio::devices(true),
            finished: false,
        }
    }
}
impl eframe::App for Preview {
    fn ui(&mut self, root: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        i18n::with_language(self.wizard.draft.language, || {
            if self.finished {
                egui::CentralPanel::default().show(root, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.heading(tr("Preview complete"));
                        ui.label(tr("No preferences were saved. No recording was started."));
                        if ui.button(tr("Try setup again")).clicked() {
                            *self = Self::new();
                        }
                        if ui.button(tr("Close preview")).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                return;
            }
            match self.wizard.show(
                root,
                Devices {
                    monitors: &self.monitors,
                    windows: &self.windows,
                    inputs: &self.inputs,
                },
            ) {
                Some(Action::Skip | Action::Finish) => {
                    self.wizard.mic_test = None;
                    self.finished = true;
                }
                Some(Action::Refresh) => {
                    self.monitors = sources::monitors();
                    self.windows = sources::windows();
                    self.inputs = audio::devices(true);
                }
                Some(Action::BrowseFolder) => {
                    match platform::choose_clip_folder(std::path::Path::new(
                        self.wizard.folder.trim(),
                    )) {
                        Ok(Some(path)) => self.wizard.folder = path.to_string_lossy().into_owned(),
                        Ok(None) => {}
                        Err(error) => self.wizard.error = error.to_string(),
                    }
                }
                None => {}
            }
        });
    }
}

pub(crate) fn run() -> Result<()> {
    unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        )
        .ok()?;
    }
    let result = eframe::run_native(
        "Gavnel · Setup preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 780.0])
                .with_min_inner_size([1000.0, 650.0])
                .with_icon(egui::IconData {
                    rgba: crate::ui::icon_rgba(),
                    width: 32,
                    height: 32,
                }),
            ..Default::default()
        },
        Box::new(|cc| {
            super::super::theme::apply(&cc.egui_ctx);
            Ok(Box::new(Preview::new()))
        }),
    );
    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }
    result.map_err(|error| anyhow::anyhow!("Could not open setup preview: {error}"))
}
