use super::views::*;
use super::*;
use egui::Align;
impl Gavnel {
    pub(super) fn settings_recording(&mut self, ui: &mut egui::Ui, focus_hotkeys: bool) {
        section(ui, "Recording Mode", |ui| {
            self.recording_mode_controls(ui, &self.engine.snapshot());
        });
        ui.add_space(14.0);
        if focus_hotkeys {
            ui.scroll_to_cursor(Some(Align::Min));
        }
        self.recording_hotkeys(ui);
        ui.add_space(14.0);
        section(ui, "Recording quality", |ui| {
            ui.label(RichText::new("Recording resolution").strong());
            super::widgets::dropdown(
                ui,
                "recording-resolution",
                &mut self.settings.height,
                &[(720, "720p"), (1080, "1080p"), (1440, "1440p")],
            );
            ui.add_space(12.0);
            ui.label(RichText::new("Frame rate").strong());
            super::widgets::dropdown(
                ui,
                "recording-fps",
                &mut self.settings.fps,
                &[(30, "30 FPS"), (60, "60 FPS"), (120, "120 FPS")],
            );
            ui.add_space(12.0);
            ui.label(RichText::new("Video bitrate").strong());
            super::widgets::slider(ui, &mut self.settings.bitrate_mbps, 5..=50, "Mbps");
        });
        ui.add_space(14.0);
    }
}
