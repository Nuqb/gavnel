use super::*;
use crate::highlights::{EventKind, Game};
impl Gavnel {
    pub(super) fn highlight_settings(&mut self, ui: &mut egui::Ui) {
        views::section(ui, tr("Automatic highlights"), |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Before"));
                ui.add(
                    egui::DragValue::new(&mut self.settings.highlights.before)
                        .range(1..=120)
                        .suffix(" s"),
                );
                ui.label(tr("After"));
                ui.add(
                    egui::DragValue::new(&mut self.settings.highlights.after)
                        .range(0..=60)
                        .suffix(" s"),
                );
            });
            let snapshot = self.engine.snapshot();
            if !snapshot.highlight_status.is_empty() {
                ui.label(tr(&snapshot.highlight_status));
            }
            let calibration_key = PathBuf::from("__gavnel-hud-calibration__");
            if let Some(image) = &snapshot.highlight_preview {
                if let Some(texture) = self.textures.get_mut(&calibration_key) {
                    texture.set((**image).clone(), egui::TextureOptions::LINEAR);
                } else {
                    self.textures.insert(
                        calibration_key.clone(),
                        ui.ctx().load_texture(
                            "hud-calibration",
                            (**image).clone(),
                            egui::TextureOptions::LINEAR,
                        ),
                    );
                }
            }
            let calibration_texture = self.textures.get(&calibration_key).cloned();
            let mut connect = false;
            for config in &mut self.settings.highlights.games {
                ui.push_id(config.game, |ui| {
                    ui.horizontal(|ui| {
                        super::checkbox::checkbox(ui, &mut config.enabled, config.game.label());
                        if config.game == Game::Cs2 {
                            connect |= ui.button(tr("Connect CS2")).clicked();
                        } else {
                            ui.label(tr("Experimental"));
                        }
                    });
                    if config.enabled {
                        ui.horizontal_wrapped(|ui| {
                            let choices: Vec<EventKind> = if config.game == Game::Cs2 {
                                vec![
                                    EventKind::Kill,
                                    EventKind::Death,
                                    EventKind::Assist,
                                    EventKind::RoundStart,
                                    EventKind::RoundWin,
                                    EventKind::RoundLoss,
                                    EventKind::MatchStart,
                                    EventKind::MatchEnd,
                                ]
                            } else {
                                config.rules.iter().map(|r| r.kind).collect()
                            };
                            for kind in choices {
                                let mut selected = config.events.contains(&kind);
                                if super::checkbox::checkbox(ui, &mut selected, tr(kind.label()))
                                    .changed()
                                {
                                    config.events.retain(|k| *k != kind);
                                    if selected {
                                        config.events.push(kind);
                                    }
                                }
                            }
                        });
                        if config.game != Game::Cs2 {
                            egui::CollapsingHeader::new(tr("HUD calibration")).show(ui, |ui| {
                                if snapshot.highlight_game == Some(config.game) {
                                    self.engine.preview_requested.store(true, Ordering::Relaxed);
                                    ui.ctx().request_repaint_after(Duration::from_millis(500));
                                    let id = ui.make_persistent_id("calibration-event");
                                    let mut selected = ui
                                        .ctx()
                                        .data_mut(|d| d.get_temp::<usize>(id).unwrap_or(0))
                                        .min(config.rules.len().saturating_sub(1));
                                    super::motion::ComboBox::from_id_salt("calibration-rule")
                                        .selected_text(tr(config
                                            .rules
                                            .get(selected)
                                            .map(|r| r.kind.label())
                                            .unwrap_or("Event")))
                                        .show_ui(ui, |ui| {
                                            for (index, rule) in config.rules.iter().enumerate() {
                                                ui.selectable_value(
                                                    &mut selected,
                                                    index,
                                                    tr(rule.kind.label()),
                                                );
                                            }
                                        });
                                    ui.ctx().data_mut(|d| d.insert_temp(id, selected));
                                    if let (Some(rule), Some(texture)) = (
                                        config.rules.get_mut(selected),
                                        calibration_texture.as_ref(),
                                    ) {
                                        calibration_canvas(ui, rule, texture);
                                    }
                                } else {
                                    ui.label(tr(
                                        "Start capture for this game to calibrate its HUD.",
                                    ));
                                }
                                for rule in &mut config.rules {
                                    ui.push_id(rule.kind, |ui| {
                                        ui.label(RichText::new(tr(rule.kind.label())).strong());
                                        let mut phrase = rule.phrases.join(", ");
                                        if ui
                                            .text_edit_singleline(&mut phrase)
                                            .on_hover_text(tr("Comma-separated HUD phrases"))
                                            .changed()
                                        {
                                            let parsed: Vec<_> = phrase
                                                .split(',')
                                                .map(str::trim)
                                                .filter(|s| !s.is_empty())
                                                .map(str::to_owned)
                                                .collect();
                                            if !parsed.is_empty() {
                                                rule.phrases = parsed;
                                            }
                                        }
                                        ui.horizontal(|ui| {
                                            for (index, label) in ["X", "Y", "Width", "Height"]
                                                .into_iter()
                                                .enumerate()
                                            {
                                                ui.label(label);
                                                ui.add(
                                                    egui::DragValue::new(&mut rule.region[index])
                                                        .speed(0.005)
                                                        .range(if index < 2 {
                                                            0.0..=0.98
                                                        } else {
                                                            0.02..=1.0
                                                        }),
                                                );
                                            }
                                        });
                                        rule.region[2] = rule.region[2].min(1.0 - rule.region[0]);
                                        rule.region[3] = rule.region[3].min(1.0 - rule.region[1]);
                                    });
                                }
                            });
                        }
                    }
                });
                ui.separator();
            }
            if connect {
                match platform::choose_clip_folder(&self.settings.clips_dir) {
                    Ok(Some(folder)) => match crate::highlights::cs2::connect(
                        &folder,
                        &mut self.settings.highlights,
                    ) {
                        Ok(_) => {
                            self.message =
                                tr("CS2 connected. Restart the game to load the integration.")
                                    .into()
                        }
                        Err(e) => self.message = e.to_string(),
                    },
                    Ok(None) => {}
                    Err(e) => self.message = e.to_string(),
                }
            }
        });
    }
}

fn calibration_canvas(
    ui: &mut egui::Ui,
    rule: &mut crate::highlights::VisualRule,
    texture: &egui::TextureHandle,
) {
    let size = texture.resolution_vec2();
    let width = ui.available_width().min(600.0);
    let height = width * size.y / size.x;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::drag());
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    let [x, y, w, h] = rule.region;
    let region = egui::Rect::from_min_size(
        rect.min + egui::vec2(x * width, y * height),
        egui::vec2(w * width, h * height),
    );
    ui.painter().rect_stroke(
        region,
        0.0,
        egui::Stroke::new(2.0, ACCENT),
        egui::StrokeKind::Inside,
    );
    let id = response.id.with("selection-start");
    if response.drag_started()
        && let Some(pos) = response.interact_pointer_pos()
    {
        ui.ctx().data_mut(|d| d.insert_temp(id, pos));
    }
    if response.dragged()
        && let Some(pos) = response.interact_pointer_pos()
        && let Some(start) = ui.ctx().data_mut(|d| d.get_temp::<egui::Pos2>(id))
    {
        let area = egui::Rect::from_two_pos(start, pos).intersect(rect);
        if area.width() >= 8.0 && area.height() >= 8.0 {
            rule.region = [
                (area.left() - rect.left()) / width,
                (area.top() - rect.top()) / height,
                area.width() / width,
                area.height() / height,
            ];
        }
    }
    response.on_hover_cursor(egui::CursorIcon::Crosshair);
}
