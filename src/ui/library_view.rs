use super::views::*;
use super::*;
use egui::{Align, Layout, Rect, Sense, Stroke, StrokeKind};
impl Gavnel {
    pub(super) fn clips(&mut self, ui: &mut egui::Ui, status: &Status) {
        let all = self.library.clips.lock().unwrap().clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Library")).size(27.0).strong());
            library_search(ui, &mut self.search);
        });
        ui.add_space(17.0);
        ui.horizontal(|ui| {
            if tab(ui, tr("All clips"), !self.settings.library_group_by_game).clicked() {
                self.settings.library_group_by_game = false;
            }
            if tab(ui, tr("By game"), self.settings.library_group_by_game).clicked() {
                self.settings.library_group_by_game = true;
            }
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
                    tr("All games")
                } else {
                    &self.game_filter
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.game_filter, String::new(), tr("All games"));
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
                    tr("Change card size"),
                )
                .clicked()
                {
                    self.settings.library_compact = !self.settings.library_compact;
                }
                super::motion::ComboBox::from_id_salt("clip-sort")
                    .width(150.0)
                    .selected_text(match self.settings.library_sort {
                        Sort::Newest => tr("Newest first"),
                        Sort::Oldest => tr("Oldest first"),
                        Sort::Largest => tr("Largest first"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings.library_sort,
                            Sort::Newest,
                            tr("Newest first"),
                        );
                        ui.selectable_value(
                            &mut self.settings.library_sort,
                            Sort::Oldest,
                            tr("Oldest first"),
                        );
                        ui.selectable_value(
                            &mut self.settings.library_sort,
                            Sort::Largest,
                            tr("Largest first"),
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
                ui.heading(tr("No matching clips"));
                ui.label(RichText::new(tr("Try another game or search term.")).color(MUTED));
                if ui.button(tr("Clear filters")).clicked() {
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
                RichText::new(tr("Your moments belong here"))
                    .size(25.0)
                    .strong(),
            );
            ui.add_space(3.0);
            ui.label(
                RichText::new(tr("Start a game. Make a play. Press your clip key."))
                    .size(14.0)
                    .color(MUTED),
            );
            ui.add_space(17.0);
            ui.horizontal(|ui| {
                let total = 245.0;
                ui.add_space((ui.available_width() - total).max(0.0) / 2.0);
                keycap(ui, &self.settings.hotkey);
                ui.label(
                    RichText::new(i18n::message(
                        "saves your last {seconds} s",
                        &[("seconds", &self.settings.replay_seconds.to_string())],
                    ))
                    .color(MUTED),
                );
            });
            ui.add_space(23.0);
            if orange_button(
                ui,
                if status.armed {
                    tr("Open capture controls")
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
                RichText::new(tr(
                    "Hundreds of supported games · Add custom games in Settings",
                ))
                .size(11.0)
                .color(Color32::from_gray(108)),
            );
        });
    }
}

/// Position both contents from the final bar rectangle, independent of row growth.
fn library_search(ui: &mut egui::Ui, search: &mut String) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::hover());
    ui.painter().rect(
        rect,
        6.0,
        PANEL,
        Stroke::new(1.0_f32, LINE),
        StrokeKind::Inside,
    );
    let icon = Rect::from_center_size(
        egui::pos2(rect.left() + 18.0, rect.center().y),
        Vec2::splat(16.0),
    );
    paint_icon(ui.painter(), icon, Glyph::Search, MUTED);
    let text = Rect::from_min_max(
        egui::pos2(rect.left() + 36.0, rect.top() + 1.0),
        egui::pos2(rect.right() - 12.0, rect.bottom() - 1.0),
    );
    ui.put(
        text,
        egui::TextEdit::singleline(search)
            .hint_text(tr("Search your clips"))
            .align(egui::Align2::LEFT_CENTER)
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE),
    );
    rect
}

#[cfg(test)]
mod search_tests {
    use super::*;
    fn shapes(shape: &egui::Shape) -> Vec<&egui::Shape> {
        match shape {
            egui::Shape::Vec(children) => children.iter().flat_map(shapes).collect(),
            _ => vec![shape],
        }
    }
    #[test]
    fn search_contents_stay_centered_without_shrinking_the_bar() {
        for width in [520.0, 1160.0] {
            for value in ["", "Valorant"] {
                let ctx = egui::Context::default();
                super::super::theme::apply(&ctx);
                let mut search = value.to_owned();
                let mut bar = Rect::NOTHING;
                let mut remaining = 0.0;
                let mut render = |ui: &mut egui::Ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Library").size(27.0).strong());
                            remaining = ui.available_width();
                            bar = library_search(ui, &mut search);
                        });
                    });
                };
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 100.0),
                    )),
                    ..Default::default()
                };
                let _ = super::super::test_render::run(&ctx, input.clone(), &mut render);
                let output = super::super::test_render::run(&ctx, input, &mut render);
                assert_eq!(bar.height(), 48.0);
                assert!((bar.width() - remaining).abs() < 1.0);
                let drawn: Vec<_> = output
                    .shapes
                    .iter()
                    .flat_map(|s| shapes(&s.shape))
                    .collect();
                let text = drawn
                    .iter()
                    .find_map(|s| match s {
                        egui::Shape::Text(text)
                            if text.galley.text()
                                == if value.is_empty() {
                                    "Search your clips"
                                } else {
                                    value
                                } =>
                        {
                            Some(text.pos.y + text.galley.size().y / 2.0)
                        }
                        _ => None,
                    })
                    .expect("Search text must be visible");
                let icon = drawn
                    .iter()
                    .find_map(|s| match s {
                        egui::Shape::Mesh(mesh)
                            if (mesh.calc_bounds().width() - 16.0).abs() < 0.1 =>
                        {
                            Some(mesh.calc_bounds().center().y)
                        }
                        _ => None,
                    })
                    .expect("Search icon must be visible");
                assert!(
                    (text - bar.center().y).abs() <= 1.0,
                    "text center {text}, bar center {}",
                    bar.center().y
                );
                assert!((icon - bar.center().y).abs() <= 0.1);
            }
        }
    }
}
