use super::*;
impl Gavnel {
    pub(super) fn open_player(&mut self, path: &std::path::Path) {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            if let Err(error) = platform::open(path) {
                self.message = i18n::message(
                    "Could not open screenshot: {error}",
                    &[("error", &format!("{error:#}"))],
                );
            }
            return;
        }
        match crate::player::Player::new(
            path.to_owned(),
            self.parent_window,
            self.context.clone(),
            self.visible.clone(),
        ) {
            Ok(player) => {
                self.player = Some(player);
                self.page = Page::Player;
                self.quick_open = false;
            }
            Err(e) => {
                self.message = i18n::message(
                    "Could not open the clip: {error}",
                    &[("error", &format!("{e:#}"))],
                )
            }
        }
    }
    pub(super) fn playback(&mut self, ui: &mut egui::Ui) {
        let mut back = false;
        if !self.quick_open
            && !ui.ctx().egui_wants_keyboard_input()
            && let Some(player) = &self.player
        {
            let keys = ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                )
            });
            if keys.0 {
                let _ = player.toggle();
            }
            if keys.1 {
                let _ = player.seek(player.position() - 5.0);
            }
            if keys.2 {
                let _ = player.seek(player.position() + 5.0);
            }
            back = keys.3;
        }
        ui.horizontal(|ui| {
            back |= ui.button(tr("‹  Library")).clicked();
            if let Some(player) = &self.player {
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            player
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                        )
                        .strong(),
                    )
                    .truncate(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr("Show in folder")).clicked() {
                        let _ = platform::reveal(&player.path);
                    }
                    if ui.button(tr("Open externally")).clicked() {
                        let _ = player.pause();
                        let _ = platform::open(&player.path);
                    }
                });
            }
        });
        ui.add_space(16.0);
        let Some(player) = &mut self.player else {
            return;
        };
        let size = Vec2::new(
            ui.available_width(),
            (ui.available_height()
                - if player.bookmarks.is_empty() {
                    56.0
                } else {
                    104.0
                })
            .max(120.0),
        );
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect_filled(rect, 6.0, Color32::BLACK);
        let error = player.error();
        if let Err(e) = player.layout(
            rect,
            ui.ctx().pixels_per_point(),
            !self.quick_open && error.is_none(),
        ) {
            self.message = i18n::message(
                "Player display failed: {error}",
                &[("error", &e.to_string())],
            );
        }
        if let Some(error) = error {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                error,
                egui::FontId::proportional(13.0),
                MUTED,
            );
        }
        ui.add_space(12.0);
        let duration = player.duration();
        let mut position = player
            .seek_preview
            .unwrap_or_else(|| player.position())
            .min(duration);
        ui.horizontal(|ui| {
            let (glyph, label) = if player.ended() {
                (views::Glyph::Refresh, tr("Replay"))
            } else if player.paused() {
                (views::Glyph::Play, tr("Play"))
            } else {
                (views::Glyph::Pause, tr("Pause"))
            };
            if views::icon_button(ui, glyph, TEXT, label).clicked()
                && let Err(e) = player.toggle()
            {
                self.message = e.to_string();
            }
            ui.label(format!("{} / {}", time(position), time(duration)));
            ui.add_enabled_ui(duration > 0.0, |ui| {
                let width = (ui.available_width() - 145.0).max(80.0);
                let response =
                    widgets::playback_slider(ui, &mut position, 0.0..=duration.max(0.01), width);
                if duration > 0.0 {
                    let track = response.rect.shrink2(Vec2::new(7.0, 0.0));
                    for seconds in &player.bookmarks {
                        let x = track.left()
                            + track.width() * (seconds / duration).clamp(0.0, 1.0) as f32;
                        ui.painter().line_segment(
                            [egui::pos2(x, track.top()), egui::pos2(x, track.bottom())],
                            egui::Stroke::new(2.0_f32, ACCENT),
                        );
                    }
                }
                if response.changed() {
                    player.seek_preview = Some(position);
                }
                if (response.drag_stopped() || (response.changed() && !response.dragged()))
                    && let Some(position) = player.seek_preview.take()
                    && let Err(e) = player.seek(position)
                {
                    self.message = e.to_string();
                }
            });
            if views::icon_button(
                ui,
                if player.muted {
                    views::Glyph::Muted
                } else {
                    views::Glyph::Volume
                },
                TEXT,
                if player.muted {
                    tr("Unmute")
                } else {
                    tr("Mute")
                },
            )
            .clicked()
            {
                player.muted = !player.muted;
                let _ = player.audio();
            }
            if widgets::playback_slider(ui, &mut player.volume, 0.0..=1.0, 90.0).changed() {
                let _ = player.audio();
            }
        });
        if !player.bookmarks.is_empty() {
            egui::ScrollArea::horizontal()
                .max_height(36.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("Bookmarks")).color(ACCENT));
                        for (index, seconds) in player.bookmarks.iter().enumerate() {
                            if ui
                                .add_enabled(
                                    duration > 0.0,
                                    egui::Button::new(format!(
                                        "{} · {}",
                                        index + 1,
                                        time(*seconds)
                                    )),
                                )
                                .on_hover_text(tr("Jump to this bookmarked moment"))
                                .clicked()
                            {
                                player.seek_preview = None;
                                if let Err(error) = player.seek(*seconds) {
                                    self.message = error.to_string();
                                }
                            }
                        }
                    });
                });
        }
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        if back {
            self.player = None;
            self.page = Page::Clips;
        }
    }
}
fn time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}
