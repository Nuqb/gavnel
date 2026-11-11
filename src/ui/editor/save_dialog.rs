use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    None,
    Close,
    Current,
    New,
}

impl Editor {
    pub(super) fn save_modal(&mut self, ctx: &egui::Context) -> Action {
        let response = egui::Modal::new(egui::Id::unique("save-video"))
            .backdrop_color(Color32::from_black_alpha(180))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .corner_radius(16.0)
                    .inner_margin(26.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 14],
                        blur: 36,
                        spread: 0,
                        color: Color32::from_black_alpha(110),
                    }),
            )
            .show(ctx, |ui| {
                ui.set_width(460.0_f32.min(ctx.content_rect().width() - 64.0));
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0);
                let close = self.save_heading(ui);
                ui.add_space(8.0);
                self.save_settings(ui);
                ui.add_space(10.0);
                if !self.error.is_empty() {
                    ui.colored_label(DANGER, &self.error);
                }
                let action = self.save_actions(ui);
                if close { Action::Close } else { action }
            });
        if response.should_close() {
            Action::Close
        } else {
            response.inner
        }
    }

    fn save_heading(&self, ui: &mut egui::Ui) -> bool {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new(tr("Save video")).size(24.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    icon_button(
                        ui,
                        "save-close",
                        include_bytes!("../../../assets/icons/close.svg"),
                        "Close",
                    )
                    .clicked()
                })
                .inner
            },
        )
        .inner
    }

    fn save_settings(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(CARD)
            .corner_radius(10.0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Resolution")).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(
                            ui,
                            "save-edit",
                            include_bytes!("../../../assets/icons/edit.svg"),
                            if self.save_editing { "Done" } else { "Edit" },
                        )
                        .clicked()
                        {
                            self.save_editing = !self.save_editing;
                        }
                    });
                });
                if self.save_editing {
                    self.resolution_control(ui);
                } else {
                    ui.label(RichText::new(self.resolution_text()).size(18.0).strong());
                }
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);
                ui.label(RichText::new(tr("File size")).color(MUTED));
                if self.save_editing {
                    self.file_size_control(ui);
                } else {
                    ui.label(RichText::new(self.file_size_text()).size(18.0).strong());
                }
            });
    }

    fn resolution_text(&self) -> String {
        let Some(first) = self.project.as_ref().and_then(|p| p.segments.first()) else {
            return tr("Original").into();
        };
        let (_, _, width, height) = first.crop.pixels(&first.info);
        let output_height = self.height.unwrap_or(height).min(height).max(2) & !1;
        let output_width =
            ((width as f64 * output_height as f64 / height as f64).round() as u32).max(2) & !1;
        format!("{output_width} \u{00d7} {output_height}")
    }

    fn resolution_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (height, label) in [
                (None, "Original"),
                (Some(1080), "1080p"),
                (Some(720), "720p"),
            ] {
                ui.selectable_value(&mut self.height, height, tr(label));
            }
        });
    }

    fn file_size_text(&self) -> String {
        self.project
            .as_ref()
            .and_then(|project| crate::media::estimated_size_mb(project, self.size_mb).ok())
            .map(|size| format!("\u{2248} {size:.1} MB"))
            .unwrap_or_else(|| "\u{2014}".into())
    }

    fn file_size_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            super::super::motion::ComboBox::from_id_salt("save-size")
                .selected_text(
                    self.size_mb
                        .map(|size| format!("{size:.0} MB"))
                        .unwrap_or_else(|| tr("Automatic").into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.size_mb, None, tr("Automatic"));
                    for size in [10.0, 25.0, 50.0, 100.0] {
                        ui.selectable_value(&mut self.size_mb, Some(size), format!("{size:.0} MB"));
                    }
                    if ui.selectable_label(false, tr("Custom")).clicked() {
                        self.size_mb = Some(25.0);
                    }
                });
            if let Some(size) = &mut self.size_mb {
                ui.add(
                    egui::DragValue::new(size)
                        .range(1.0..=10000.0)
                        .suffix(" MB"),
                );
            }
        });
    }

    fn save_actions(&self, ui: &mut egui::Ui) -> Action {
        ui.horizontal(|ui| {
            let width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            let current = ui
                .add_enabled_ui(self.current_video().is_some(), |ui| {
                    ui.add_sized([width, 42.0], egui::Button::new(tr("Save current video")))
                })
                .inner
                .on_hover_text(tr("Replace the current video with these edits"))
                .on_disabled_hover_text(tr("Combined videos must be saved as a new video"));
            let new = ui.add_sized(
                [width, 42.0],
                egui::Button::new(RichText::new(tr("Save as new video")).color(Color32::BLACK))
                    .fill(ACCENT),
            );
            if current.clicked() {
                Action::Current
            } else if new.clicked() {
                Action::New
            } else {
                Action::None
            }
        })
        .inner
    }
}

fn icon_button(
    ui: &mut egui::Ui,
    name: &'static str,
    svg: &'static [u8],
    label: &str,
) -> egui::Response {
    let texture = crate::icons::texture(ui.ctx(), name, svg, 32);
    let image = egui::Image::new((texture.id(), Vec2::splat(18.0))).tint(TEXT);
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = Vec2::splat(4.0);
            ui.spacing_mut().interact_size = Vec2::splat(28.0);
            ui.add_sized(
                [28.0, 28.0],
                egui::Button::image(image).frame_when_inactive(false),
            )
        })
        .inner
        .on_hover_text(tr(label));
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tr(label)));
    response
}
