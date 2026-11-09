use super::views::{Glyph, icon_button, paint_icon};
use super::*;
use egui::{Align, Align2, FontId, Layout, Rect, Sense, Stroke, StrokeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Game,
    Desktop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Automatic,
    Window(usize),
    Monitor(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Close,
    Refresh,
    Confirm(Choice),
}
pub(super) struct SourcePicker {
    tab: Tab,
    manual: bool,
    window: Option<usize>,
    monitor: Option<usize>,
    previews: HashMap<usize, egui::TextureHandle>,
}
impl SourcePicker {
    fn new(settings: &Settings, windows: &[WindowInfo], monitors: &[MonitorInfo]) -> Self {
        Self {
            tab: if settings.mode == CaptureMode::Monitor {
                Tab::Desktop
            } else {
                Tab::Game
            },
            manual: settings.mode == CaptureMode::Window,
            window: windows
                .iter()
                .find(|w| {
                    w.title == settings.window_title && w.executable == settings.window_executable
                })
                .map(|w| w.handle),
            monitor: monitors
                .iter()
                .find(|m| m.index == settings.monitor_index)
                .or(monitors.first())
                .map(|m| m.index),
            previews: HashMap::new(),
        }
    }
    fn choice(&self, windows: &[WindowInfo], monitors: &[MonitorInfo]) -> Option<Choice> {
        match self.tab {
            Tab::Game if !self.manual => Some(Choice::Automatic),
            Tab::Game => self
                .window
                .filter(|id| windows.iter().any(|w| w.handle == *id))
                .map(Choice::Window),
            Tab::Desktop => self
                .monitor
                .filter(|id| monitors.iter().any(|m| m.index == *id))
                .map(Choice::Monitor),
        }
    }
    fn load_previews(&mut self, ctx: &egui::Context, monitors: &[MonitorInfo]) {
        self.previews.clear();
        for monitor in monitors {
            if let Ok(image) = sources::monitor_preview(monitor.index) {
                self.previews.insert(
                    monitor.index,
                    ctx.load_texture(
                        format!("display-preview-{}", monitor.index),
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                );
            }
        }
    }
    fn show(
        &mut self,
        ctx: &egui::Context,
        windows: &[WindowInfo],
        monitors: &[MonitorInfo],
        active_source: &str,
        confirm_label: &str,
    ) -> Option<Action> {
        let response = egui::Modal::new(egui::Id::new("capture-source-picker"))
            .backdrop_color(Color32::from_black_alpha(175))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .corner_radius(10.0)
                    .inner_margin(24.0),
            )
            .show(ctx, |ui| {
                ui.set_width(460.0);
                let mut action = None;
                ui.columns(2, |columns| {
                    for (ui, (tab, label)) in columns
                        .iter_mut()
                        .zip([(Tab::Game, "Game"), (Tab::Desktop, "Desktop")])
                    {
                        let selected = self.tab == tab;
                        let response = ui.add_sized(
                            [ui.available_width(), 36.0],
                            egui::Button::new(
                                RichText::new(label).size(18.0).strong().color(if selected {
                                    TEXT
                                } else {
                                    MUTED
                                }),
                            )
                            .frame_when_inactive(false),
                        );
                        if response.clicked() {
                            self.tab = tab;
                        }
                        let rect = response.rect;
                        ui.painter().line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            Stroke::new(2.0_f32, if selected { TEXT } else { LINE }),
                        );
                    }
                });
                ui.add_space(18.0);
                match self.tab {
                    Tab::Game if !self.manual => {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(460.0, 140.0), Sense::click());
                        let hovered = response.hovered();
                        ui.painter().rect_filled(
                            rect,
                            9.0,
                            if hovered {
                                Color32::from_rgb(48, 48, 54)
                            } else {
                                CARD
                            },
                        );
                        if hovered {
                            ui.painter().rect_stroke(
                                rect,
                                9.0,
                                Stroke::new(1.0_f32, ACCENT),
                                StrokeKind::Inside,
                            );
                        }
                        paint_icon(
                            ui.painter(),
                            Rect::from_center_size(
                                egui::pos2(rect.center().x, rect.top() + 43.0),
                                Vec2::splat(25.0),
                            ),
                            Glyph::Game,
                            if hovered { TEXT } else { MUTED },
                        );
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.top() + 86.0),
                            Align2::CENTER_CENTER,
                            if active_source.is_empty() {
                                "Waiting for game to be detected"
                            } else {
                                active_source
                            },
                            FontId::proportional(15.0),
                            TEXT,
                        );
                        if response
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.manual = true;
                        }
                    }
                    Tab::Game => {
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            super::motion::ComboBox::from_id_salt("source-game-process")
                                .width(395.0)
                                .selected_text(
                                    self.window
                                        .and_then(|id| windows.iter().find(|w| w.handle == id))
                                        .map(|w| w.title.as_str())
                                        .unwrap_or("Select your game process…"),
                                )
                                .show_ui(ui, |ui| {
                                    for window in windows {
                                        ui.selectable_value(
                                            &mut self.window,
                                            Some(window.handle),
                                            format!("{} · {}", window.title, window.executable),
                                        );
                                    }
                                });
                            if icon_button(ui, Glyph::Refresh, MUTED, "Refresh running programs")
                                .clicked()
                            {
                                action = Some(Action::Refresh);
                            }
                        });
                        ui.add_space(12.0);
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Automatic game detection").color(ACCENT),
                                )
                                .frame_when_inactive(false),
                            )
                            .clicked()
                        {
                            self.manual = false;
                        }
                        ui.add_space(35.0);
                    }
                    Tab::Desktop => {
                        egui::ScrollArea::horizontal()
                            .id_salt("source-monitors")
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for monitor in monitors {
                                        let selected = self.monitor == Some(monitor.index);
                                        let (rect, response) = ui.allocate_exact_size(
                                            Vec2::new(223.0, 140.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(rect, 8.0, CARD);
                                        if let Some(texture) = self.previews.get(&monitor.index) {
                                            ui.painter().image(
                                                texture.id(),
                                                rect.shrink(2.0),
                                                Rect::from_min_max(
                                                    egui::Pos2::ZERO,
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                Color32::WHITE,
                                            );
                                        } else {
                                            paint_icon(
                                                ui.painter(),
                                                Rect::from_center_size(
                                                    rect.center(),
                                                    Vec2::splat(36.0),
                                                ),
                                                Glyph::Record,
                                                MUTED,
                                            );
                                        }
                                        let band = Rect::from_min_max(
                                            egui::pos2(rect.left() + 2.0, rect.bottom() - 34.0),
                                            rect.right_bottom() - Vec2::splat(2.0),
                                        );
                                        ui.painter().rect_filled(
                                            band,
                                            2.0,
                                            Color32::from_black_alpha(200),
                                        );
                                        ui.painter().text(
                                            band.left_center() + Vec2::new(10.0, 0.0),
                                            Align2::LEFT_CENTER,
                                            format!("DISPLAY {}", monitor.index),
                                            FontId::proportional(14.0),
                                            TEXT,
                                        );
                                        ui.painter().rect_stroke(
                                            rect,
                                            8.0,
                                            Stroke::new(
                                                2.0_f32,
                                                if selected {
                                                    ACCENT
                                                } else if response.hovered() {
                                                    TEXT
                                                } else {
                                                    LINE
                                                },
                                            ),
                                            StrokeKind::Inside,
                                        );
                                        if response
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.monitor = Some(monitor.index);
                                        }
                                    }
                                });
                            });
                        if monitors.is_empty() {
                            ui.label("No displays available");
                        }
                    }
                }
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [180.0, 36.0],
                            egui::Button::new("Close").frame_when_inactive(false),
                        )
                        .clicked()
                    {
                        action = Some(Action::Close);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let choice = self.choice(windows, monitors);
                        if ui
                            .add_enabled(
                                choice.is_some(),
                                egui::Button::new(RichText::new(confirm_label).strong().color(BG))
                                    .fill(TEXT)
                                    .min_size(Vec2::new(210.0, 36.0)),
                            )
                            .clicked()
                        {
                            action = choice.map(Action::Confirm);
                        }
                    });
                });
                action
            });
        response
            .inner
            .or_else(|| response.should_close().then_some(Action::Close))
    }
}
impl Gavnel {
    pub(super) fn open_source_picker(&mut self) {
        self.quick_open = false;
        self.windows = sources::windows();
        self.monitors = sources::monitors();
        let mut picker = SourcePicker::new(&self.settings, &self.windows, &self.monitors);
        picker.load_previews(&self.context, &self.monitors);
        self.source_picker = Some(picker);
    }
    pub(super) fn show_source_picker(&mut self, ctx: &egui::Context, status: &Status) {
        let label = if status.armed {
            "Continue"
        } else if self.settings.recording_mode.full() {
            "Start Recording"
        } else {
            "Start Clipping"
        };
        let action = self.source_picker.as_mut().and_then(|picker| {
            picker.show(ctx, &self.windows, &self.monitors, &status.source, label)
        });
        match action {
            Some(Action::Close) => {
                self.source_picker = None;
            }
            Some(Action::Refresh) => {
                self.windows = sources::windows();
                self.monitors = sources::monitors();
                if let Some(picker) = &mut self.source_picker {
                    picker.load_previews(ctx, &self.monitors);
                }
            }
            Some(Action::Confirm(choice)) => {
                let previous = self.settings.clone();
                match choice {
                    Choice::Automatic => {
                        self.settings.mode = CaptureMode::Automatic;
                    }
                    Choice::Monitor(index) => {
                        self.settings.mode = CaptureMode::Monitor;
                        self.settings.monitor_index = index;
                    }
                    Choice::Window(handle) => {
                        let Some(window) = self.windows.iter().find(|w| w.handle == handle) else {
                            return;
                        };
                        self.settings.mode = CaptureMode::Window;
                        self.settings.window_title = window.title.clone();
                        self.settings.window_executable = window.executable.clone();
                    }
                }
                if self.settings.recording_mode.automatic()
                    && self.settings.mode != CaptureMode::Automatic
                {
                    self.settings.recording_mode = RecordingMode::FullRecording;
                }
                if self.settings.recording_mode == RecordingMode::Off {
                    self.settings.recording_mode = RecordingMode::Clips;
                }
                match self.apply() {
                    Ok(()) => {
                        if !status.armed {
                            self.send(Command::Start(Box::new(self.settings.clone())));
                        }
                        self.source_picker = None;
                    }
                    Err(error) => {
                        self.settings = previous;
                        self.message = error.to_string();
                    }
                }
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> (Vec<WindowInfo>, Vec<MonitorInfo>) {
        (
            vec![WindowInfo {
                handle: 42,
                title: "Test Game".into(),
                executable: "test.exe".into(),
            }],
            vec![
                MonitorInfo {
                    index: 1,
                    label: "Display 1".into(),
                },
                MonitorInfo {
                    index: 2,
                    label: "Display 2".into(),
                },
            ],
        )
    }
    fn frame(
        ctx: &egui::Context,
        picker: &mut SourcePicker,
        events: Vec<egui::Event>,
    ) -> (Option<Action>, egui::CursorIcon) {
        let (windows, monitors) = data();
        let mut action = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(900.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("Background settings");
                });
                action = picker.show(ctx, &windows, &monitors, "", "Continue");
            },
        );
        (action, output.platform_output.cursor_icon)
    }
    fn click(ctx: &egui::Context, picker: &mut SourcePicker, pos: egui::Pos2) -> Option<Action> {
        frame(
            ctx,
            picker,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            ctx,
            picker,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        )
        .0
    }
    #[test]
    fn waiting_card_has_hover_affordance_opens_process_picker_and_escape_closes() {
        let ctx = egui::Context::default();
        let (windows, monitors) = data();
        let mut picker = SourcePicker::new(&Settings::default(), &windows, &monitors);
        for _ in 0..3 {
            frame(&ctx, &mut picker, vec![]);
        }
        let rect = ctx.memory(|memory| {
            memory
                .area_rect(egui::Id::new("capture-source-picker"))
                .unwrap()
        });
        let pos = egui::pos2(rect.center().x, rect.top() + 145.0);
        let (_, cursor) = frame(&ctx, &mut picker, vec![egui::Event::PointerMoved(pos)]);
        assert_eq!(cursor, egui::CursorIcon::PointingHand);
        click(&ctx, &mut picker, pos);
        assert!(picker.manual);
        assert_eq!(picker.choice(&windows, &monitors), None);
        picker.window = Some(42);
        assert_eq!(picker.choice(&windows, &monitors), Some(Choice::Window(42)));
        assert_eq!(picker.choice(&[], &monitors), None);
        let (action, _) = frame(
            &ctx,
            &mut picker,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(action, Some(Action::Close));
    }
    #[test]
    fn desktop_tab_selects_display_and_confirms_without_editing_saved_settings() {
        let ctx = egui::Context::default();
        let (windows, monitors) = data();
        let settings = Settings::default();
        let mut picker = SourcePicker::new(&settings, &windows, &monitors);
        for _ in 0..3 {
            frame(&ctx, &mut picker, vec![]);
        }
        let rect = ctx.memory(|memory| {
            memory
                .area_rect(egui::Id::new("capture-source-picker"))
                .unwrap()
        });
        click(
            &ctx,
            &mut picker,
            egui::pos2(rect.center().x + 115.0, rect.top() + 42.0),
        );
        assert_eq!(picker.tab, Tab::Desktop);
        frame(&ctx, &mut picker, vec![]);
        let rect = ctx.memory(|memory| {
            memory
                .area_rect(egui::Id::new("capture-source-picker"))
                .unwrap()
        });
        click(
            &ctx,
            &mut picker,
            egui::pos2(rect.center().x + 115.0, rect.top() + 135.0),
        );
        assert_eq!(picker.choice(&windows, &monitors), Some(Choice::Monitor(2)));
        assert_eq!(settings.mode, CaptureMode::Automatic);
        let action = click(
            &ctx,
            &mut picker,
            egui::pos2(rect.right() - 130.0, rect.bottom() - 42.0),
        );
        assert_eq!(action, Some(Action::Confirm(Choice::Monitor(2))));
    }
}
