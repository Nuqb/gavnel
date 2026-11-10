use super::*;

impl Wizard {
    pub(super) fn show(&mut self, root: &mut egui::Ui, devices: Devices<'_>) -> Option<Action> {
        i18n::with_language(self.draft.language, || self.render(root, devices))
    }

    fn render(&mut self, root: &mut egui::Ui, devices: Devices<'_>) -> Option<Action> {
        let ctx = &root.ctx().clone();
        super::super::fonts::ensure_language(ctx, self.draft.language.code());
        self.capture_keys(ctx);
        let now = ctx.input(|input| input.time);
        let entered = *self.entered_at.get_or_insert(now);
        let progress = ((now - entered) / 0.3).clamp(0.0, 1.0) as f32;
        if progress < 1.0 {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        let mut action = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(22.0))
            .show(root, |ui| {
                if self.step == Step::Welcome {
                    self.welcome(ui, &mut action);
                    return;
                }
                let width = ui.available_width().min(1000.0);
                let page_height = (ui.available_height() - 190.0).clamp(350.0, 440.0);
                ui.add_space(((ui.available_height() - page_height - 170.0) * 0.5).max(0.0));
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - width) * 0.5).max(0.0));
                    egui::Frame::new()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0_f32, LINE))
                        .corner_radius(20.0)
                        .inner_margin(24.0)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(width - 48.0);
                                ui.spacing_mut().item_spacing.y = 6.0;
                                ui.spacing_mut().button_padding.y = 5.0;
                                ui.horizontal(|ui| {
                                    if self.preview {
                                        ui.label(
                                            RichText::new(tr("Preview · changes aren’t saved"))
                                                .size(12.0)
                                                .color(MUTED),
                                        );
                                    }
                                    ui.with_layout(trailing(Align::Center), |ui| {
                                        if ui.button(tr("Skip setup")).clicked() {
                                            action = Some(Action::Skip);
                                        }
                                    });
                                });
                                ui.add_space(10.0);
                                ui.horizontal_top(|ui| {
                                    if i18n::is_rtl() {
                                        self.page_content(
                                            ui,
                                            (width, page_height, progress),
                                            &devices,
                                            &mut action,
                                        );
                                        ui.add_space(28.0);
                                        self.rail(ui);
                                    } else {
                                        self.rail(ui);
                                        ui.add_space(28.0);
                                        self.page_content(
                                            ui,
                                            (width, page_height, progress),
                                            &devices,
                                            &mut action,
                                        );
                                    }
                                });
                                if !self.error.is_empty() {
                                    ui.colored_label(Color32::LIGHT_RED, tr(&self.error));
                                }
                                ui.add_space(12.0);
                                ui.separator();
                                ui.add_space(10.0);
                                self.footer(ui, &mut action);
                            });
                        });
                });
            });
        super::super::motion::finish(ctx);
        action
    }

    fn welcome(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.add_space((ui.available_height() * 0.5 - 145.0).max(0.0));
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 270.0) * 0.5).max(0.0));
                super::super::language::selector(ui, &mut self.draft.language, "welcome-language");
            });
            ui.add_space(24.0);
            ui.heading(RichText::new(tr("Welcome to Gavnel.")).size(36.0));
            ui.add_space(10.0);
            ui.label(RichText::new(tr("Want to set up?")).size(20.0).color(MUTED));
            ui.add_space(28.0);
            if ui
                .add_sized(
                    [180.0, 44.0],
                    egui::Button::new(RichText::new(tr("Let’s set up")).strong().color(BG))
                        .fill(ACCENT),
                )
                .clicked()
            {
                self.advance();
            }
            ui.add_space(8.0);
            if ui
                .add_sized(
                    [180.0, 36.0],
                    egui::Button::new(tr("Skip setup")).frame(false),
                )
                .clicked()
            {
                *action = Some(Action::Skip);
            }
        });
    }

    fn rail(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.set_width(160.0);
            for step in Step::ALL.into_iter().skip(1) {
                let current = self.step == step;
                if ui
                    .add_sized(
                        [160.0, 38.0],
                        egui::Button::new(RichText::new(step.label()).color(if current {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .fill(if current {
                            ACCENT_DARK
                        } else {
                            Color32::TRANSPARENT
                        })
                        .frame(current),
                    )
                    .clicked()
                    && step.index() < self.step.index()
                {
                    self.go_to(step);
                }
                ui.add_space(4.0);
            }
        });
    }

    fn page_content(
        &mut self,
        ui: &mut egui::Ui,
        dimensions: (f32, f32, f32),
        devices: &Devices<'_>,
        action: &mut Option<Action>,
    ) {
        let (width, height, progress) = dimensions;
        ui.vertical(|ui| {
            ui.set_width(width - 256.0);
            ui.set_min_height(height);
            ui.heading(RichText::new(self.step.label()).size(24.0));
            ui.add_space(10.0);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.set_opacity(0.35 + progress * 0.65);
                egui::ScrollArea::vertical()
                    .id_salt("setup-content")
                    .max_height((height - 48.0).max(100.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| self.content(ui, devices, action));
            });
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.columns(2, |columns| {
            let (back, next) = if i18n::is_rtl() { (1, 0) } else { (0, 1) };
            columns[back].allocate_ui_with_layout(
                Vec2::new(columns[back].available_width(), 38.0),
                leading(Align::Center),
                |ui| {
                    if ui
                        .add_sized([90.0, 38.0], egui::Button::new(tr("Back")))
                        .clicked()
                    {
                        self.go_to(Step::ALL[self.step.index() - 1]);
                    }
                    ui.label(
                        RichText::new(i18n::message(
                            "{current} of {total}",
                            &[
                                ("current", &(self.step.index() + 1).to_string()),
                                ("total", &Step::ALL.len().to_string()),
                            ],
                        ))
                        .color(MUTED)
                        .size(12.0),
                    );
                },
            );
            columns[next].allocate_ui_with_layout(
                Vec2::new(columns[next].available_width(), 38.0),
                trailing(Align::Center),
                |ui| {
                    let label = if self.step == Step::Ready {
                        tr("Finish setup")
                    } else {
                        tr("Continue")
                    };
                    if ui
                        .add_enabled(
                            self.listening.is_none(),
                            egui::Button::new(RichText::new(label).strong().color(BG))
                                .fill(ACCENT)
                                .min_size(Vec2::new(150.0, 38.0)),
                        )
                        .clicked()
                    {
                        if self.step == Step::Ready {
                            match self.candidate() {
                                Ok(_) => *action = Some(Action::Finish),
                                Err(error) => self.error = error.to_string(),
                            }
                        } else {
                            self.advance();
                        }
                    }
                },
            );
        });
    }
}

pub(super) fn leading(align: Align) -> Layout {
    if i18n::is_rtl() {
        Layout::right_to_left(align)
    } else {
        Layout::left_to_right(align)
    }
}
pub(super) fn trailing(align: Align) -> Layout {
    if i18n::is_rtl() {
        Layout::left_to_right(align)
    } else {
        Layout::right_to_left(align)
    }
}
