use super::views::*;
use super::*;
use std::{collections::BTreeMap, sync::Arc};

const GAME_BROWSER: &str = "supported-game-browser";

impl Gavnel {
    pub(super) fn settings_games(&mut self, ui: &mut egui::Ui) {
        let browser_id = egui::Id::unique(GAME_BROWSER);
        section(ui, tr("Game detection"), |ui| {
            description(
                ui,
                tr("Automatic detection"),
                tr(
                    "Gavnel recognizes thousands of games automatically. Browse the full list to find yours.",
                ),
            );
            if ui.button(tr("View supported games")).clicked() {
                ui.data_mut(|data| data.insert_temp(browser_id, true));
            }
        });
        ui.add_space(14.0);
        self.custom_game_list(ui);
    }

    fn custom_game_list(&mut self, ui: &mut egui::Ui) {
        let mut remove = None;
        let mut add = false;
        section_with_header(
            ui,
            |ui| {
                ui.label(RichText::new(tr("Your custom games")).size(16.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    add = ui.button(format!("+ {}", tr("Add game"))).clicked();
                });
            },
            |ui| {
                let mut any = false;
                for (index, profile) in self.settings.profiles.iter().enumerate() {
                    if profile.is_official() {
                        continue;
                    }
                    any = true;
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&profile.name).strong());
                            ui.label(RichText::new(&profile.executable).size(11.0).color(MUTED));
                        });
                        if ui.small_button(tr("Remove")).clicked() {
                            remove = Some(index);
                        }
                    });
                    ui.add_space(5.0);
                }
                if !any {
                    ui.label(RichText::new(tr("No custom games added yet.")).color(MUTED));
                }
            },
        );
        if add {
            self.add_game_dialog = Some(Default::default());
        }
        if let Some(index) = remove {
            self.settings.profiles.remove(index);
        }
        ui.add_space(14.0);
    }

    pub(super) fn game_browser_open(&self, ui: &mut egui::Ui) -> bool {
        ui.data_mut(|data| {
            data.get_temp::<bool>(egui::Id::unique(GAME_BROWSER))
                .unwrap_or(false)
        })
    }

    pub(super) fn supported_game_browser(&self, ui: &mut egui::Ui) {
        if ui.button(tr("← Back to game detection")).clicked() {
            ui.data_mut(|data| data.insert_temp(egui::Id::unique(GAME_BROWSER), false));
            return;
        }
        ui.add_space(12.0);
        ui.label(RichText::new(tr("Supported games")).size(23.0).strong());
        ui.add_space(12.0);
        let search_id = egui::Id::unique("supported-game-search");
        let mut search = ui.data_mut(|data| data.get_temp::<String>(search_id).unwrap_or_default());
        let changed = game_search(ui, &mut search);
        ui.data_mut(|data| data.insert_temp(search_id, search.clone()));
        ui.add_space(14.0);
        let games = cached_games(ui, &self.settings.profiles, &search);
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("supported-game-cards")
            .auto_shrink([false, false]);
        if changed {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        let columns = ((ui.available_width() / 240.0) as usize).clamp(1, 3);
        if games.is_empty() {
            ui.label(tr(
                "No matching games. Try another name or add a custom game.",
            ));
            return;
        }
        scroll.show_rows(ui, 92.0, games.len().div_ceil(columns), |ui, rows| {
            let start = rows.start * columns;
            let end = (rows.end * columns).min(games.len());
            game_grid(ui, &games[start..end], columns);
        });
    }
}

#[derive(Clone)]
struct GameCard {
    name: String,
    executables: Vec<String>,
}

#[derive(Clone)]
struct BrowserCache {
    profiles: Arc<[crate::config::GameProfile]>,
    query: String,
    games: Arc<[GameCard]>,
}

fn cached_games(
    ui: &mut egui::Ui,
    profiles: &[crate::config::GameProfile],
    search: &str,
) -> Arc<[GameCard]> {
    let id = egui::Id::unique("supported-game-cache");
    let cached = ui.data_mut(|data| data.get_temp::<BrowserCache>(id));
    if let Some(cache) = cached
        && cache.profiles.as_ref() == profiles
        && cache.query == search
    {
        return cache.games;
    }
    let games: Arc<[GameCard]> = grouped_games(profiles, search).into();
    ui.data_mut(|data| {
        data.insert_temp(
            id,
            BrowserCache {
                profiles: profiles.into(),
                query: search.into(),
                games: games.clone(),
            },
        )
    });
    games
}

fn grouped_games(profiles: &[crate::config::GameProfile], search: &str) -> Vec<GameCard> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for profile in profiles {
        groups
            .entry(profile.name.clone())
            .or_default()
            .push(profile.executable.clone());
    }
    let query = search.trim().to_lowercase();
    groups
        .into_iter()
        .filter(|(name, executables)| {
            name.to_lowercase().contains(&query)
                || executables
                    .iter()
                    .any(|exe| exe.to_lowercase().contains(&query))
        })
        .map(|(name, executables)| GameCard { name, executables })
        .collect()
}

fn game_card(ui: &mut egui::Ui, game: &GameCard) {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0_f32, LINE))
        .corner_radius(9.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_min_size(Vec2::new(ui.available_width(), 66.0));
            ui.add(egui::Label::new(RichText::new(&game.name).strong()).truncate());
            ui.label(
                RichText::new(format!(
                    "{} executable{}",
                    game.executables.len(),
                    if game.executables.len() == 1 { "" } else { "s" }
                ))
                .size(11.0)
                .color(MUTED),
            );
        })
        .response
        .on_hover_text(game.executables.join("\n"));
}

fn game_search(ui: &mut egui::Ui, search: &mut String) -> bool {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0_f32, LINE))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                paint_icon(ui.painter(), rect, Glyph::Search, MUTED);
                ui.add(
                    egui::TextEdit::singleline(search)
                        .hint_text(tr("Search games or executable names"))
                        .align(egui::Align2::LEFT_CENTER)
                        .frame(egui::Frame::NONE)
                        .desired_width(ui.available_width()),
                )
                .changed()
            })
            .inner
        })
        .inner
}

fn game_grid(ui: &mut egui::Ui, games: &[GameCard], columns: usize) {
    if games.is_empty() {
        ui.label(tr(
            "No matching games. Try another name or add a custom game.",
        ));
        return;
    }
    for row in games.chunks(columns) {
        ui.columns(columns, |uis| {
            for (column, game) in uis.iter_mut().zip(row) {
                game_card(column, game);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_row_leaves_room_for_games_at_different_window_sizes() {
        for size in [Vec2::new(640.0, 480.0), Vec2::new(1280.0, 944.0)] {
            let ctx = egui::Context::default();
            super::super::theme::apply(&ctx);
            let mut search = String::new();
            let mut games_top = 0.0;
            let _ = super::super::test_render::run(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        game_search(ui, &mut search);
                        games_top = ui.label("Game results").rect.top();
                    });
                },
            );
            assert!(
                games_top < 80.0,
                "Search consumed the page: results start at {games_top}"
            );
        }
    }
}
