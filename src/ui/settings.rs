use super::views::*;
use super::*;
use egui::{Align, Layout};
impl Gavnel {
    pub(super) fn settings(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(860.0);
        let mut rect = ui.available_rect_before_wrap();
        rect.min.x += (rect.width() - width) * 0.5;
        rect.max.x = rect.min.x + width;
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.set_width(width);
                self.settings_content(ui);
            },
        );
    }

    fn settings_content(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Settings").size(27.0).strong());
        ui.add_space(17.0);
        ui.horizontal(|ui| {
            for (page, label) in [
                (SettingsPage::Recording, "Recording"),
                (SettingsPage::Audio, "Audio"),
                (SettingsPage::Storage, "Storage"),
                (SettingsPage::General, "General"),
                (SettingsPage::Games, "Game detection"),
            ] {
                if tab(ui, label, self.settings_page == page).clicked() {
                    self.settings_page = page;
                }
            }
        });
        ui.separator();
        ui.add_space(14.0);
        let mut scroll = egui::ScrollArea::vertical().id_salt("settings-scroll");
        let focus_hotkeys = ui.ctx().data_mut(|data| {
            data.remove_temp::<bool>(egui::Id::new("focus-hotkeys"))
                .unwrap_or(false)
        });
        if ui.ctx().data_mut(|data| {
            data.remove_temp::<bool>(egui::Id::new("focus-recording-mode"))
                .unwrap_or(false)
        }) || focus_hotkeys
        {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll.show(ui, |ui| {
            match self.settings_page {
                SettingsPage::Recording => self.settings_recording(ui, focus_hotkeys),
                SettingsPage::Audio => self.settings_audio(ui),
                SettingsPage::Storage => self.settings_storage(ui),
                SettingsPage::General => self.settings_general(ui),
                SettingsPage::Games => self.settings_games(ui),
            }
            ui.add_space(18.0);
            if !matches!(self.settings_page, SettingsPage::Audio | SettingsPage::Storage) { ui.label(
                RichText::new("Source, quality, or audio changes save the current full recording and begin a new file. Replay clips start a fresh buffer.")
                    .color(MUTED)
                    .size(12.0),
            ); }
        });
    }
}
