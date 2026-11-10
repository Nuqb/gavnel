use super::*;
use egui::{Align, Layout, Pos2, Rect, Sense, Stroke};

#[derive(Clone, Copy)]
pub(super) enum Glyph {
    Library,
    Game,
    Settings,
    Folder,
    Search,
    Mic,
    Volume,
    Grid,
    Play,
    Pause,
    Muted,
    Refresh,
    Record,
}
impl Gavnel {
    pub(super) fn draw(&mut self, root: &mut egui::Ui, status: &Status) {
        let previous = (self.page, self.settings_page);
        let rail = if i18n::is_rtl() {
            egui::Panel::right("app-rail")
        } else {
            egui::Panel::left("app-rail")
        };
        rail.exact_size(66.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(8.0))
            .show(root, |ui| {
                ui.add_space(7.0);
                let (logo, _) = ui.allocate_exact_size(Vec2::new(48.0, 40.0), Sense::hover());
                crate::branding::paint(ui.painter(), logo.shrink(1.0), self.logo_phase());
                ui.add_space(20.0);
                if nav(
                    ui,
                    Glyph::Library,
                    self.page == Page::Clips,
                    tr("Clip library"),
                )
                .clicked()
                {
                    self.page = Page::Clips;
                }
                ui.add_space(6.0);
                if nav(
                    ui,
                    Glyph::Game,
                    self.page == Page::Settings && self.settings_page == SettingsPage::Games,
                    tr("Game detection"),
                )
                .clicked()
                {
                    self.page = Page::Settings;
                    self.settings_page = SettingsPage::Games;
                }
                ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                    ui.add_space(8.0);
                    if nav(
                        ui,
                        Glyph::Settings,
                        self.page == Page::Settings && self.settings_page != SettingsPage::Games,
                        tr("Settings"),
                    )
                    .clicked()
                    {
                        self.page = Page::Settings;
                        self.settings_page = SettingsPage::Recording;
                    }
                    ui.add_space(6.0);
                    if nav(ui, Glyph::Folder, false, tr("Open clips folder")).clicked() {
                        self.open_folder();
                    }
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(env!("CARGO_PKG_VERSION"))
                            .size(10.0)
                            .color(MUTED),
                    );
                });
            });
        egui::Panel::top("recording-toolbar")
            .exact_size(66.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(root, |ui| self.toolbar(ui, status));
        self.guard_editor_navigation(previous);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(28.0))
            .show(root, |ui| {
                if self.page != Page::Settings && !self.message.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(tr(&self.message)).color(ACCENT).size(12.0));
                        if ui.small_button(tr("Dismiss")).clicked() {
                            self.message.clear();
                        }
                    });
                    ui.add_space(8.0);
                }
                self.update_notice(ui);
                match self.page {
                    Page::Player => self.playback(ui),
                    Page::Editor => self.edit_view(ui),
                    Page::Clips => self.clips(ui, status),
                    Page::Capture => {
                        egui::ScrollArea::vertical().show(ui, |ui| self.capture(ui, status));
                    }
                    Page::Settings => self.settings(ui),
                }
            });
        self.guard_editor_navigation(previous);
        self.editor_leave_dialog(root.ctx());
    }
}

pub(super) fn mono_controls(ui: &mut egui::Ui, settings: &mut Settings) -> bool {
    let before = settings.mic_mono;
    audio_toggle(ui, tr("Mono Audio Input"), &mut settings.mic_mono);
    before != settings.mic_mono
}

pub(super) fn audio_toggle(ui: &mut egui::Ui, label: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(label)).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            super::widgets::toggle(ui, value);
            ui.label(
                RichText::new(if *value { tr("ON") } else { tr("OFF") })
                    .size(11.0)
                    .color(MUTED),
            );
        });
    });
    ui.add_space(8.0);
}

pub(super) fn audio_volume_row(
    ui: &mut egui::Ui,
    label: &str,
    glyph: Glyph,
    enabled: &mut bool,
    volume: &mut f32,
) {
    ui.horizontal(|ui| {
        super::checkbox::checkbox(ui, enabled, "");
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
        paint_icon(ui.painter(), rect, glyph, MUTED);
        ui.label(RichText::new(tr(label)).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled_ui(*enabled, |ui| {
                super::widgets::slider(ui, volume, 0.0..=2.0, "%");
            });
        });
    });
}
pub(super) fn orange_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(tr(text))
                .strong()
                .color(Color32::from_rgb(28, 16, 7)),
        )
        .fill(ACCENT)
        .min_size(Vec2::new(118.0, 35.0)),
    )
}
pub(super) fn badge(ui: &mut egui::Ui, text: &str, bg: Color32, color: Color32) {
    egui::Frame::new()
        .fill(bg)
        .corner_radius(5.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(tr(text)).size(10.0).color(color).strong());
        });
}
pub(super) fn keycap(ui: &mut egui::Ui, text: &str) {
    let text = crate::key_labels::display(text);
    egui::Frame::new()
        .fill(Color32::from_rgb(42, 42, 47))
        .stroke(Stroke::new(1.0_f32, Color32::from_gray(65)))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).strong());
        });
}
pub(super) fn tab(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    let response = ui.add(
        egui::Button::new(
            RichText::new(tr(text))
                .size(13.0)
                .strong()
                .color(if selected { ACCENT } else { MUTED }),
        )
        .frame_when_inactive(false)
        .min_size(Vec2::new(78.0, 33.0)),
    );
    if selected {
        let r = response.rect;
        ui.painter().line_segment(
            [
                Pos2::new(r.left() + 9.0, r.bottom()),
                Pos2::new(r.right() - 9.0, r.bottom()),
            ],
            Stroke::new(2.0_f32, ACCENT),
        );
    }
    response
}
pub(super) fn nav(ui: &mut egui::Ui, glyph: Glyph, selected: bool, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(48.0, 48.0), Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            9.0,
            if response.hovered() {
                Color32::from_rgb(51, 45, 41)
            } else if selected {
                ACCENT_DARK
            } else {
                CARD
            },
        );
    }
    if selected {
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() - 8.0, rect.top() + 12.0),
                Vec2::new(3.0, 24.0),
            ),
            1.0,
            ACCENT,
        );
    }
    paint_icon(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(23.0)),
        glyph,
        if selected { ACCENT } else { MUTED },
    );
    response.on_hover_text(tr(label))
}
pub(super) fn icon_button(
    ui: &mut egui::Ui,
    glyph: Glyph,
    color: Color32,
    label: &str,
) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(Vec2::splat(31.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(r, 5.0, Color32::from_rgb(48, 48, 54));
    }
    paint_icon(
        ui.painter(),
        Rect::from_center_size(r.center(), Vec2::splat(18.0)),
        glyph,
        color,
    );
    response.on_hover_text(tr(label))
}
pub(super) fn section(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    section_with_header(
        ui,
        |ui| {
            ui.label(RichText::new(tr(title)).size(16.0).strong());
            if title == tr("Recording Mode") {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    super::widgets::recording_mode_help(ui);
                });
            }
        },
        content,
    );
}

pub(super) fn settings_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(8.0)
        .inner_margin(egui::Margin {
            left: 20,
            right: 20,
            top: 10,
            bottom: 16,
        })
}
pub(super) fn section_with_header(
    ui: &mut egui::Ui,
    header: impl FnOnce(&mut egui::Ui),
    content: impl FnOnce(&mut egui::Ui),
) {
    settings_panel_frame().show(ui, |ui| {
        ui.set_min_width((ui.available_width() - 1.0).max(0.0));
        ui.horizontal(header);
        ui.add_space(8.0);
        content(ui);
    });
}
pub(super) fn description(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.label(RichText::new(tr(title)).strong());
    ui.label(RichText::new(tr(detail)).color(MUTED).size(12.0));
    ui.add_space(4.0);
}
pub(super) fn device_selector(
    ui: &mut egui::Ui,
    id: &str,
    selected: &mut String,
    devices: &[AudioDevice],
) {
    super::motion::ComboBox::from_id_salt(id)
        .width(ui.available_width().min(380.0))
        .selected_text(
            devices
                .iter()
                .find(|d| d.id == *selected)
                .map(|d| d.label.as_str())
                .unwrap_or(if selected.is_empty() {
                    tr("Windows default device")
                } else {
                    tr("Selected device is disconnected")
                }),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, String::new(), tr("Windows default device"));
            for d in devices {
                ui.selectable_value(selected, d.id.clone(), &d.label);
            }
        });
}
pub(super) fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}
pub(super) fn paint_icon(p: &egui::Painter, rect: Rect, glyph: Glyph, color: Color32) {
    let (name, svg): (&'static str, &'static [u8]) = match glyph {
        Glyph::Library => (
            "nav-library",
            include_bytes!("../../assets/icons/library.svg"),
        ),
        Glyph::Game => (
            "nav-gamepad",
            include_bytes!("../../assets/icons/gamepad.svg"),
        ),
        Glyph::Settings => (
            "nav-settings",
            include_bytes!("../../assets/icons/settings.svg"),
        ),
        Glyph::Folder => (
            "nav-folder",
            include_bytes!("../../assets/icons/folder.svg"),
        ),
        Glyph::Search => (
            "nav-search",
            include_bytes!("../../assets/icons/search.svg"),
        ),
        Glyph::Mic => (
            "nav-microphone",
            include_bytes!("../../assets/icons/microphone.svg"),
        ),
        Glyph::Volume => (
            "nav-volume",
            include_bytes!("../../assets/icons/volume.svg"),
        ),
        Glyph::Grid => ("nav-grid", include_bytes!("../../assets/icons/grid.svg")),
        Glyph::Pause => (
            "player-pause",
            include_bytes!("../../assets/icons/pause.svg"),
        ),
        Glyph::Muted => (
            "player-muted",
            include_bytes!("../../assets/icons/muted.svg"),
        ),
        Glyph::Play => ("nav-play", include_bytes!("../../assets/icons/play.svg")),
        Glyph::Refresh => (
            "nav-refresh",
            include_bytes!("../../assets/icons/refresh.svg"),
        ),
        Glyph::Record => (
            "nav-capture",
            include_bytes!("../../assets/icons/capture.svg"),
        ),
    };
    crate::icons::paint(p, name, svg, rect, color);
}
