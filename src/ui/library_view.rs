use super::views::*;
use super::*;
use egui::{Align, Layout, Rect, Sense, Stroke, StrokeKind};
impl Gavnel {
    pub(super) fn clips(&mut self, ui: &mut egui::Ui, status: &Status) {
        let all = self.library.clips.lock().unwrap().clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Library").size(27.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, Glyph::Folder, MUTED, "Open clips folder").clicked() {
                    self.open_folder();
                }
                if icon_button(ui, Glyph::Refresh, MUTED, "Refresh library").clicked() {
                    self.library.refresh(&self.settings.clips_dir);
                }
                let width = (ui.available_width() - 120.0).clamp(145.0, 280.0);
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                            paint_icon(ui.painter(), r, Glyph::Search, MUTED);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .hint_text("Search your clips")
                                    .frame(false)
                                    .desired_width(width),
                            );
                        });
                    });
            });
        });
        ui.add_space(17.0);
        ui.horizontal(|ui| {
            if tab(ui, "All clips", !self.settings.library_group_by_game).clicked() {
                self.settings.library_group_by_game = false;
            }
            if tab(ui, "By game", self.settings.library_group_by_game).clicked() {
                self.settings.library_group_by_game = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("On this PC").color(MUTED).size(12.0));
                let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                paint_icon(ui.painter(), r, Glyph::Folder, MUTED);
            });
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(9.0);
        ui.horizontal(|ui| {
            let mut games: Vec<_> = all.iter().map(|c| c.source.clone()).collect();
            games.sort();
            games.dedup();
            super::motion::ComboBox::from_id_salt("game-filter")
                .width(175.0)
                .selected_text(if self.game_filter.is_empty() {
                    "All games"
                } else {
                    &self.game_filter
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.game_filter, String::new(), "All games");
                    for game in games {
                        ui.selectable_value(&mut self.game_filter, game.clone(), game);
                    }
                });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(
                    ui,
                    Glyph::Grid,
                    if self.settings.library_compact {
                        ACCENT
                    } else {
                        MUTED
                    },
                    "Change card size",
                )
                .clicked()
                {
                    self.settings.library_compact = !self.settings.library_compact;
                }
                super::motion::ComboBox::from_id_salt("clip-sort")
                    .width(150.0)
                    .selected_text(match self.settings.library_sort {
                        Sort::Newest => "Newest first",
                        Sort::Oldest => "Oldest first",
                        Sort::Largest => "Largest first",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings.library_sort,
                            Sort::Newest,
                            "Newest first",
                        );
                        ui.selectable_value(
                            &mut self.settings.library_sort,
                            Sort::Oldest,
                            "Oldest first",
                        );
                        ui.selectable_value(
                            &mut self.settings.library_sort,
                            Sort::Largest,
                            "Largest first",
                        );
                    });
            });
        });
        ui.add_space(17.0);
        let query = self.search.to_lowercase();
        let mut clips: Vec<_> = all
            .iter()
            .filter(|c| {
                (self.game_filter.is_empty() || c.source == self.game_filter)
                    && (query.is_empty()
                        || c.name.to_lowercase().contains(&query)
                        || c.source.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();
        match self.settings.library_sort {
            Sort::Newest => clips.sort_by_key(|c| std::cmp::Reverse(c.modified)),
            Sort::Oldest => clips.sort_by_key(|c| c.modified),
            Sort::Largest => clips.sort_by_key(|c| std::cmp::Reverse(c.bytes)),
        }
        if all.is_empty() {
            self.empty_library(ui, status);
            return;
        }
        if clips.is_empty() {
            ui.add_space(70.0);
            ui.vertical_centered(|ui| {
                ui.heading("No matching clips");
                ui.label(RichText::new("Try another game or search term.").color(MUTED));
                if ui.button("Clear filters").clicked() {
                    self.search.clear();
                    self.game_filter.clear();
                }
            });
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.settings.library_group_by_game {
                    let mut names: Vec<_> = clips.iter().map(|c| c.source.clone()).collect();
                    names.sort();
                    names.dedup();
                    for name in names {
                        ui.label(RichText::new(&name).size(17.0).strong());
                        ui.add_space(7.0);
                        let subset: Vec<_> =
                            clips.iter().filter(|c| c.source == name).cloned().collect();
                        self.clip_grid(ui, &subset);
                        ui.add_space(22.0);
                    }
                } else {
                    self.clip_grid(ui, &clips);
                }
            });
    }

    fn empty_library(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.add_space((ui.available_height() * 0.14).max(25.0));
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(130.0, 100.0), Sense::hover());
            let back =
                Rect::from_center_size(r.center() + Vec2::new(10.0, -8.0), Vec2::new(88.0, 63.0));
            ui.painter().rect_filled(back, 8.0, CARD);
            ui.painter()
                .rect_stroke(back, 8.0, Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
            let front =
                Rect::from_center_size(r.center() + Vec2::new(-5.0, 7.0), Vec2::new(88.0, 63.0));
            ui.painter()
                .rect_filled(front, 8.0, Color32::from_rgb(34, 28, 24));
            ui.painter().rect_stroke(
                front,
                8.0,
                Stroke::new(1.0_f32, Color32::from_rgb(98, 61, 35)),
                StrokeKind::Inside,
            );
            paint_icon(
                ui.painter(),
                Rect::from_center_size(front.center(), Vec2::splat(28.0)),
                Glyph::Play,
                ACCENT,
            );
            ui.add_space(14.0);
            ui.label(
                RichText::new("Your moments belong here")
                    .size(25.0)
                    .strong(),
            );
            ui.add_space(3.0);
            ui.label(
                RichText::new("Start a game. Make a play. Press your clip key.")
                    .size(14.0)
                    .color(MUTED),
            );
            ui.add_space(17.0);
            ui.horizontal(|ui| {
                let total = 245.0;
                ui.add_space((ui.available_width() - total).max(0.0) / 2.0);
                keycap(ui, &self.settings.hotkey);
                ui.label(
                    RichText::new(format!(
                        "saves your last {} seconds",
                        self.settings.replay_seconds
                    ))
                    .color(MUTED),
                );
            });
            ui.add_space(23.0);
            if orange_button(
                ui,
                if status.armed {
                    "Open capture controls"
                } else {
                    self.recording_button(status)
                },
            )
            .clicked()
            {
                if status.armed {
                    self.page = Page::Capture;
                } else {
                    self.toggle_recording(status);
                }
            }
            ui.add_space(24.0);
            ui.label(
                RichText::new("Fortnite   /   Valorant   /   Rust   /   Counter-Strike 2")
                    .size(11.0)
                    .color(Color32::from_gray(108)),
            );
        });
    }
}
