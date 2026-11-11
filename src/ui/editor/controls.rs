use super::*;
use crate::media::Aspect;
impl Editor {
    pub(super) fn clip_strip(&mut self, ui: &mut egui::Ui) {
        let Some(p) = &mut self.project else {
            return;
        };
        egui::ScrollArea::horizontal()
            .max_height(42.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut movement = None;
                    let mut remove = false;
                    for (i, s) in p.segments.iter().enumerate() {
                        let label = format!(
                            "{}  {}",
                            i + 1,
                            s.info
                                .path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                        );
                        if ui.selectable_label(self.selected == i, label).clicked() {
                            self.selected = i;
                            self.cursor = s.start;
                            self.last_frame = None;
                        }
                    }
                    if p.segments.len() > 1 {
                        if ui.button("←").on_hover_text(tr("Move earlier")).clicked()
                            && self.selected > 0
                        {
                            movement = Some(self.selected - 1);
                        }
                        if ui.button("→").on_hover_text(tr("Move later")).clicked()
                            && self.selected + 1 < p.segments.len()
                        {
                            movement = Some(self.selected + 1);
                        }
                        remove = ui.button(tr("Remove")).clicked();
                    }
                    if let Some(other) = movement {
                        p.segments.swap(self.selected, other);
                        self.selected = other;
                    }
                    if remove {
                        p.segments.remove(self.selected);
                        self.selected = self.selected.min(p.segments.len() - 1);
                    }
                });
            });
    }
    pub(super) fn controls(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 5.0;
        ui.spacing_mut().button_padding = Vec2::new(10.0, 5.0);
        ui.spacing_mut().interact_size.y = 26.0;
        ui.spacing_mut().slider_width = 185.0;
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Trim, "Trim"),
                (Tab::Crop, "Crop"),
                (Tab::Audio, "Audio"),
            ] {
                let icon = match tab {
                    Tab::Trim => buttons::Icon::Trim,
                    Tab::Crop => buttons::Icon::Crop,
                    Tab::Audio => buttons::Icon::Audio,
                };
                if buttons::tab(ui, icon, label, self.tab == tab).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();
        let Some(s) = self
            .project
            .as_mut()
            .and_then(|p| p.segments.get_mut(self.selected))
        else {
            return;
        };
        match self.tab {
            Tab::Trim => {
                ui.columns(2, |columns| {
                    columns[0].label(tr("Start"));
                    columns[0].add(
                        egui::DragValue::new(&mut s.start)
                            .range(0.0..=(s.end - 1.0 / s.info.fps).max(0.0))
                            .speed(0.05)
                            .fixed_decimals(2)
                            .suffix(" s"),
                    );
                    columns[1].label(tr("End"));
                    columns[1].add(
                        egui::DragValue::new(&mut s.end)
                            .range((s.start + 1.0 / s.info.fps)..=s.info.duration)
                            .speed(0.05)
                            .fixed_decimals(2)
                            .suffix(" s"),
                    );
                });
                if buttons::button(ui, buttons::Icon::Reset, "Reset trim").clicked() {
                    s.start = 0.0;
                    s.end = s.info.duration;
                }
                ui.add_space(10.0);
                for (time, label) in s.info.markers.clone() {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                false,
                                format!("{}  {}", playback::time(time), tr(&label)),
                            )
                            .clicked()
                        {
                            self.cursor = time;
                        }
                        if ui.small_button(tr("Create clip")).clicked() {
                            s.bookmark(time);
                            self.cursor = s.start;
                        }
                    });
                }
            }
            Tab::Crop => {
                ui.horizontal_wrapped(|ui| {
                    for aspect in [
                        Aspect::Original,
                        Aspect::Wide,
                        Aspect::Vertical,
                        Aspect::Square,
                        Aspect::Custom,
                    ] {
                        if ui
                            .selectable_label(s.crop.aspect == aspect, tr(aspect.label()))
                            .clicked()
                        {
                            if aspect == Aspect::Custom {
                                let rect = s.crop.normalized(&s.info);
                                s.crop.set_rect(rect);
                            } else {
                                s.crop.aspect = aspect;
                                s.crop.custom_rect = None;
                            }
                        }
                    }
                });
                ui.add_space(8.0);
                if buttons::button(ui, buttons::Icon::Center, "Center crop").clicked() {
                    s.crop.center();
                }
            }
            Tab::Audio => {
                if s.audio.is_empty() {
                    ui.label(tr("No audio tracks"));
                }
                let mut reset_audio = false;
                for (index, track) in s.audio.iter_mut().enumerate() {
                    ui.push_id(track.index, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [92.0, 24.0],
                                egui::Label::new(RichText::new(tr(&track.label)).strong())
                                    .truncate(),
                            )
                            .on_hover_text(&track.label);
                            super::super::checkbox::checkbox(ui, &mut track.muted, tr("Mute"));
                            super::super::checkbox::checkbox(ui, &mut track.solo, tr("Solo"));
                        });
                        ui.horizontal(|ui| {
                            ui.scope(|ui| {
                                if track.muted {
                                    ui.multiply_opacity(0.45);
                                }
                                super::super::widgets::playback_slider(
                                    ui,
                                    &mut track.gain,
                                    0.0..=2.0,
                                    125.0,
                                );
                                ui.label(format!("{:.0}%", track.gain * 100.0));
                            });
                            if index == 0
                                && buttons::button(ui, buttons::Icon::Reset, "Reset audio")
                                    .clicked()
                            {
                                reset_audio = true;
                            }
                        });
                    });
                    ui.add_space(3.0);
                }
                if reset_audio {
                    for track in &mut s.audio {
                        track.gain = 1.0;
                        track.muted = false;
                        track.solo = false;
                    }
                }
            }
        }
    }
}
