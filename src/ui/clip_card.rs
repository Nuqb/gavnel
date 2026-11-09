use super::views::*;
use super::*;
use crate::library::Clip;
use egui::{Align, Align2, FontId, Layout, Pos2, Rect, Sense};
impl Gavnel {
    pub(super) fn clip_grid(&mut self, ui: &mut egui::Ui, clips: &[Clip]) {
        let gap = 16.0;
        let min = if self.settings.library_compact {
            240.0
        } else {
            310.0
        };
        let columns = ((ui.available_width() + gap) / (min + gap))
            .floor()
            .clamp(1.0, 5.0) as usize;
        let width = ((ui.available_width() - gap * (columns - 1) as f32) / columns as f32).floor();
        for row in clips.chunks(columns) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for clip in row {
                    ui.allocate_ui_with_layout(
                        Vec2::new(width, width * 9.0 / 16.0 + 78.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(width);
                            self.clip_card(ui, clip, width);
                        },
                    );
                }
            });
            ui.add_space(6.0);
        }
    }
    pub(super) fn clip_card(&mut self, ui: &mut egui::Ui, clip: &Clip, width: f32) {
        let visible = ui.clip_rect().intersects(Rect::from_min_size(
            ui.next_widget_position(),
            Vec2::new(width, width * 9.0 / 16.0 + 78.0),
        ));
        if visible
            && !self.textures.contains_key(&clip.path)
            && let Some(image) = self.thumbnails.get(&clip.path)
        {
            if self.textures.len() >= 96 {
                self.textures.clear();
            }
            let texture = ui.ctx().load_texture(
                clip.path.to_string_lossy(),
                (*image).clone(),
                egui::TextureOptions::LINEAR,
            );
            self.textures.insert(clip.path.clone(), texture);
        }
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(7.0)
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 0.0;
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(width, width * 9.0 / 16.0), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius {
                        nw: 7,
                        ne: 7,
                        sw: 0,
                        se: 0,
                    },
                    Color32::from_rgb(12, 12, 14),
                );
                if let Some(texture) = self.textures.get(&clip.path) {
                    let size = texture.resolution_vec2();
                    let scale = (rect.width() / size.x).min(rect.height() / size.y);
                    let dest = Rect::from_center_size(rect.center(), size * scale);
                    ui.painter().image(
                        texture.id(),
                        dest,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    paint_icon(
                        ui.painter(),
                        Rect::from_center_size(rect.center(), Vec2::splat(33.0)),
                        Glyph::Play,
                        Color32::from_gray(72),
                    );
                }
                let duration = if clip.is_screenshot() {
                    "PNG".into()
                } else {
                    format_duration(clip.duration)
                };
                let pill = Rect::from_min_size(
                    Pos2::new(rect.right() - 57.0, rect.bottom() - 30.0),
                    Vec2::new(48.0, 22.0),
                );
                ui.painter()
                    .rect_filled(pill, 4.0, Color32::from_black_alpha(220));
                ui.painter().text(
                    pill.center(),
                    Align2::CENTER_CENTER,
                    duration,
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );
                if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, 7.0, Color32::from_black_alpha(65));
                    ui.painter().circle_filled(rect.center(), 24.0, ACCENT);
                    paint_icon(
                        ui.painter(),
                        Rect::from_center_size(rect.center(), Vec2::splat(21.0)),
                        if clip.is_screenshot() {
                            Glyph::Folder
                        } else {
                            Glyph::Play
                        },
                        Color32::from_rgb(24, 15, 7),
                    );
                }
                if response.clicked() {
                    self.open_player(&clip.path);
                }
                response.context_menu(|ui| {
                    ui.spacing_mut().button_padding.x += 4.0;
                    if ui
                        .button(if clip.is_screenshot() {
                            tr("Open screenshot")
                        } else {
                            tr("Play clip")
                        })
                        .clicked()
                    {
                        self.open_player(&clip.path);
                        ui.close();
                    }
                    if ui.button(tr("Show in folder")).clicked() {
                        let _ = platform::reveal(&clip.path);
                        ui.close();
                    }
                    if super::widgets::destructive_button(
                        ui,
                        if clip.is_screenshot() {
                            tr("Delete screenshot")
                        } else {
                            tr("Delete clip")
                        },
                        false,
                    )
                    .clicked()
                    {
                        self.delete_pending = Some(clip.path.clone());
                        self.delete_error = None;
                        ui.close();
                    }
                });
                egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                    ui.set_width(width - 24.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            super::motion::vertical_menu_button(ui, |ui| {
                                ui.spacing_mut().button_padding.x += 4.0;
                                if ui.button(tr("Show in folder")).clicked() {
                                    let _ = platform::reveal(&clip.path);
                                    ui.close();
                                }
                                if super::widgets::destructive_button(
                                    ui,
                                    if clip.is_screenshot() {
                                        tr("Delete screenshot")
                                    } else {
                                        tr("Delete clip")
                                    },
                                    false,
                                )
                                .clicked()
                                {
                                    self.delete_pending = Some(clip.path.clone());
                                    self.delete_error = None;
                                    ui.close();
                                }
                            })
                            .response
                            .on_hover_text(tr("Clip actions"));
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&clip.source).size(14.0).strong(),
                                    )
                                    .truncate(),
                                );
                            });
                        });
                    });
                    ui.add_space(7.0);
                    let date: chrono::DateTime<chrono::Local> = clip.modified.into();
                    ui.label(
                        RichText::new(i18n::message(
                            "On device · {size} · {date}",
                            &[
                                ("size", &crate::storage::format_bytes(clip.bytes)),
                                ("date", &date.format("%Y-%m-%d").to_string()),
                            ],
                        ))
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            });
    }
}
