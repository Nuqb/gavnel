use super::*;
use egui::{Align, FontId, Layout, Rect, Sense, Vec2};

fn duration_label(seconds: u32) -> String {
    if seconds.is_multiple_of(60) {
        i18n::message("{minutes} min", &[("minutes", &(seconds / 60).to_string())])
    } else {
        i18n::message("{seconds} s", &[("seconds", &seconds.to_string())])
    }
}

pub(super) fn clip_button(
    ui: &mut egui::Ui,
    hotkey: &str,
    seconds: u32,
    open: bool,
) -> egui::Response {
    shortcut_button(
        ui,
        hotkey,
        &i18n::message("Clip {duration}", &[("duration", &duration_label(seconds))]),
        open,
    )
    .on_hover_text(tr("Clip hotkey and duration"))
}

pub(super) fn shortcut_button(
    ui: &mut egui::Ui,
    hotkey: &str,
    caption: &str,
    open: bool,
) -> egui::Response {
    let hotkey = if hotkey.is_empty() {
        tr("Not set")
    } else {
        hotkey
    };
    let keys: Vec<_> = hotkey
        .split('+')
        .map(|part| {
            ui.painter().layout_no_wrap(
                crate::key_labels::display(part.trim()),
                FontId::monospace(12.0),
                TEXT,
            )
        })
        .collect();
    let keys_width: f32 = keys
        .iter()
        .map(|key| (key.size().x + 12.0).max(32.0))
        .sum::<f32>()
        + 4.0 * keys.len().saturating_sub(1) as f32;
    let caption = ui
        .painter()
        .layout_no_wrap(caption.to_owned(), FontId::proportional(13.0), TEXT);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(keys_width + caption.size().x + 25.0, 38.0),
        Sense::click(),
    );
    ui.painter().rect_filled(
        rect,
        8.0,
        if response.hovered() || open {
            Color32::from_rgb(45, 45, 50)
        } else {
            CARD
        },
    );
    let mut left = rect.left() + 4.0;
    for key in keys {
        let width = (key.size().x + 12.0).max(32.0);
        let key_rect = Rect::from_center_size(
            egui::pos2(left + width / 2.0, rect.center().y),
            Vec2::new(width, 32.0),
        );
        ui.painter()
            .rect_filled(key_rect, 6.0, Color32::from_rgb(55, 55, 61));
        ui.painter()
            .galley(key_rect.center() - key.size() / 2.0, key, TEXT);
        left += width + 4.0;
    }
    ui.painter().galley(
        egui::pos2(
            rect.left() + 4.0 + keys_width + 9.0,
            rect.center().y - caption.size().y / 2.0,
        ),
        caption,
        TEXT,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub(super) fn manage_hotkeys_button(ui: &mut egui::Ui) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(tr("Manage Hotkeys  →")).size(12.0).strong())
            .frame_when_inactive(false),
    )
}
impl Gavnel {
    pub(super) fn clips_menu(
        &mut self,
        ui: &mut egui::Ui,
        anchor: &egui::Response,
        opened_from_toolbar: bool,
    ) {
        let mut open = self.clips_open;
        super::widgets::quick_panel(
            ui,
            anchor,
            egui::Id::unique("clips-settings-menu"),
            &mut open,
            opened_from_toolbar,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("CLIPS")).size(11.0).color(MUTED).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if manage_hotkeys_button(ui).clicked() {
                            self.page = Page::Settings;
                            self.settings_page = SettingsPage::Recording;
                            self.clips_open = false;
                            ui.ctx().data_mut(|data| {
                                data.insert_temp(egui::Id::unique("focus-hotkeys"), true)
                            });
                            ui.close();
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let capturing = self.capturing_hotkey.load(Ordering::Relaxed);
                    let label = if capturing {
                        tr("Press a key…")
                    } else if self.settings.hotkey.is_empty() {
                        tr("Not set")
                    } else {
                        &self.settings.hotkey
                    };
                    if ui
                        .add_sized(
                            [196.0, 40.0],
                            egui::Button::new(
                                RichText::new(crate::key_labels::display(label))
                                    .font(FontId::monospace(14.0)),
                            )
                            .fill(if capturing { ACCENT_DARK } else { BG })
                            .corner_radius(6.0),
                        )
                        .clicked()
                    {
                        self.capturing_action = self.settings.primary_clip_id();
                        self.pressed_hotkeys.lock().unwrap().clear();
                        self.capturing_hotkey.store(true, Ordering::Relaxed);
                        self.clips_capture_owned = true;
                    }
                    super::motion::ComboBox::from_id_salt("clip-menu-duration")
                        .width(126.0)
                        .selected_text(duration_label(self.settings.replay_seconds))
                        .show_ui(ui, |ui| {
                            for seconds in [5, 10, 15, 30, 45, 60, 90, 120] {
                                if ui
                                    .selectable_label(
                                        self.settings.replay_seconds == seconds,
                                        duration_label(seconds),
                                    )
                                    .clicked()
                                {
                                    self.settings.set_primary_clip_seconds(seconds);
                                }
                            }
                        });
                });
            },
        );
        self.clips_open = open && self.clips_open;
        if self.clips_capture_owned && !self.clips_open {
            self.capturing_hotkey.store(false, Ordering::Relaxed);
            self.clips_capture_owned = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> (Rect, bool, egui::CursorIcon) {
        let mut rect = Rect::NOTHING;
        let mut clicked = false;
        let output = crate::ui::test_render::run(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600.0, 200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let response = clip_button(ui, "F8", 60, false);
                    rect = response.rect;
                    clicked = response.clicked();
                });
            },
        );
        (rect, clicked, output.platform_output.cursor_icon)
    }
    #[test]
    fn keybind_and_duration_are_one_click_target_with_hover_feedback() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![]);
        let (rect, _, _) = frame(&ctx, vec![]);
        assert_eq!(rect.height(), 38.0);
        for x in [rect.left() + 15.0, rect.right() - 15.0] {
            let pos = egui::pos2(x, rect.center().y);
            let (_, _, cursor) = frame(&ctx, vec![egui::Event::PointerMoved(pos)]);
            assert_eq!(cursor, egui::CursorIcon::PointingHand);
            frame(
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
            );
            let (_, clicked, _) = frame(
                &ctx,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            assert!(clicked);
        }
    }
}
