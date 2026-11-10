use super::*;
use crate::capture_hotkeys::{
    CaptureAction, CaptureHotkey, HotkeyTrigger, LONG_PRESS_DELAY, RegisteredShortcut,
};

pub(super) fn parse_bindings(settings: &Settings) -> Result<Vec<RegisteredShortcut>> {
    let mut result: Vec<RegisteredShortcut> = Vec::new();
    for binding in settings.capture_bindings() {
        if binding.key.is_empty() {
            continue;
        }
        let key: HotKey = binding.key.parse().with_context(|| {
            i18n::message(
                "Choose a valid shortcut for {action}.",
                &[("action", tr(binding.action.label()))],
            )
        })?;
        anyhow::ensure!(
            !result
                .iter()
                .any(|old| old.key == key && old.binding.trigger == binding.trigger),
            tr(
                "That key combination and trigger are already assigned. Choose another key or trigger."
            )
        );
        result.push(RegisteredShortcut { key, binding });
    }
    Ok(result)
}

impl Gavnel {
    pub(super) fn install_shortcut_handler(&self) {
        let commands = self.engine.commands.clone();
        let capturing = self.capturing_hotkey.clone();
        let bindings = self.hotkey.clone();
        let active = self.active.clone();
        let held = self.pressed_hotkeys.clone();
        self.hotkeys
            .set_event_handler(move |event: GlobalHotKeyEvent| {
                let choices: Vec<_> = bindings
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|b| b.key.id() == event.id)
                    .cloned()
                    .collect();
                let has_long = choices
                    .iter()
                    .any(|b| b.binding.trigger == HotkeyTrigger::LongPress);
                if event.state == HotKeyState::Released {
                    let release = held.lock().unwrap().release(event.id, Instant::now());
                    if !has_long || capturing.load(Ordering::Relaxed) {
                        return;
                    }
                    let Some((duration, fired)) = release else {
                        return;
                    };
                    if fired {
                        return;
                    }
                    let trigger = if duration >= LONG_PRESS_DELAY {
                        HotkeyTrigger::LongPress
                    } else {
                        HotkeyTrigger::Tap
                    };
                    if let Some(choice) = choices.iter().find(|b| b.binding.trigger == trigger) {
                        let settings = active.lock().unwrap();
                        if settings.recording_mode != RecordingMode::Off {
                            let _ = commands.try_send(choice.binding.command(&settings));
                        }
                    }
                    return;
                }
                if choices.is_empty()
                    || capturing.load(Ordering::Relaxed)
                    || active.lock().unwrap().recording_mode == RecordingMode::Off
                {
                    return;
                }
                let Some(token) = held.lock().unwrap().press(event.id, Instant::now()) else {
                    return;
                };
                if !has_long {
                    if let Some(choice) = choices.first() {
                        let _ = commands.try_send(choice.binding.command(&active.lock().unwrap()));
                    }
                    return;
                }
                let (held, commands, capturing, bindings, active) = (
                    held.clone(),
                    commands.clone(),
                    capturing.clone(),
                    bindings.clone(),
                    active.clone(),
                );
                std::thread::spawn(move || {
                    std::thread::sleep(LONG_PRESS_DELAY);
                    if capturing.load(Ordering::Relaxed) {
                        return;
                    }
                    let choices = bindings.lock().unwrap();
                    let Some(choice) = choices.iter().find(|b| {
                        b.key.id() == event.id && b.binding.trigger == HotkeyTrigger::LongPress
                    }) else {
                        return;
                    };
                    if !held
                        .lock()
                        .unwrap()
                        .fire_long(event.id, token, Instant::now())
                    {
                        return;
                    }
                    let settings = active.lock().unwrap();
                    if settings.recording_mode != RecordingMode::Off {
                        let _ = commands.try_send(choice.binding.command(&settings));
                    }
                });
            });
    }

    pub(super) fn select_shortcut(&mut self, value: String) -> Result<()> {
        let mut candidate = self.settings.clone();
        candidate
            .edit_capture_bindings()
            .iter_mut()
            .find(|b| b.id == self.capturing_action)
            .context(tr("This hotkey was removed. Add it again to set a key."))?
            .key = value;
        parse_bindings(&candidate)?;
        candidate.sync_primary_bindings();
        self.settings = candidate;
        Ok(())
    }
    pub(super) fn toggle_long_recording(&mut self) {
        if self.settings.recording_mode == RecordingMode::Off {
            self.message = tr("Choose a recording mode to enable long recording.").into();
            return;
        }
        match self.apply() {
            Ok(()) => self.send(Command::ToggleLongRecording(Box::new(
                self.settings.clone(),
            ))),
            Err(error) => self.message = error.to_string(),
        }
    }
    pub(super) fn bind_shortcuts(&mut self) -> Result<()> {
        let new = parse_bindings(&self.settings)?;
        let mut current = self.hotkey.lock().unwrap();
        if *current == new {
            return Ok(());
        }
        let mut registered = Vec::new();
        for entry in &new {
            if !current.iter().any(|old| old.key == entry.key) && !registered.contains(&entry.key) {
                if let Err(error) = self.hotkeys.register(entry.key) {
                    for key in registered {
                        let _ = self.hotkeys.unregister(key);
                    }
                    anyhow::bail!(i18n::message(
                        "{action} shortcut is already in use. Choose another. {error}",
                        &[
                            ("action", tr(entry.binding.action.label())),
                            ("error", &error.to_string())
                        ]
                    ));
                }
                registered.push(entry.key);
            }
        }
        let mut removed = Vec::new();
        for entry in current.iter() {
            if !new.iter().any(|new| new.key == entry.key) && !removed.contains(&entry.key) {
                let _ = self.hotkeys.unregister(entry.key);
                removed.push(entry.key);
            }
        }
        *current = new;
        self.pressed_hotkeys.lock().unwrap().clear();
        Ok(())
    }
    pub(super) fn recording_hotkeys(&mut self, ui: &mut egui::Ui) {
        let capturing = self
            .capturing_hotkey
            .load(Ordering::Relaxed)
            .then_some(self.capturing_action);
        if let Some(id) = hotkeys_panel(ui, &mut self.settings, capturing) {
            self.capturing_action = id;
            self.pressed_hotkeys.lock().unwrap().clear();
            self.capturing_hotkey.store(true, Ordering::Relaxed);
        }
    }
}

fn icon(action: CaptureAction) -> (&'static str, &'static [u8], Color32) {
    match action {
        CaptureAction::Clip => (
            "hotkey-clip",
            include_bytes!("../../assets/icons/hotkey-clip.svg"),
            Color32::from_rgb(195, 235, 94),
        ),
        CaptureAction::LongRecording => (
            "hotkey-long",
            include_bytes!("../../assets/icons/hotkey-recording.svg"),
            Color32::from_rgb(83, 218, 166),
        ),
        CaptureAction::Bookmark => (
            "hotkey-bookmark",
            include_bytes!("../../assets/icons/hotkey-bookmark.svg"),
            Color32::from_rgb(163, 137, 246),
        ),
        CaptureAction::Screenshot => (
            "hotkey-screenshot",
            include_bytes!("../../assets/icons/hotkey-screenshot.svg"),
            Color32::from_rgb(75, 162, 255),
        ),
    }
}
pub(super) fn action_icon(ui: &mut egui::Ui, action: CaptureAction) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
    let (name, bytes, color) = icon(action);
    crate::icons::paint(ui.painter(), name, bytes, rect, color);
}
fn duration_label(seconds: u32) -> String {
    if seconds.is_multiple_of(60) {
        i18n::message("{minutes} min", &[("minutes", &(seconds / 60).to_string())])
    } else {
        i18n::message("{seconds} s", &[("seconds", &seconds.to_string())])
    }
}

pub(super) fn hotkeys_panel(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    capturing: Option<u64>,
) -> Option<u64> {
    settings.ensure_capture_action_rows();
    let removable_actions: Vec<_> = CaptureAction::ALL
        .into_iter()
        .filter(|action| {
            settings
                .capture_bindings()
                .iter()
                .filter(|binding| binding.action == *action)
                .count()
                > 1
        })
        .collect();
    let mut selected = None;
    let mut add = None;
    let mut remove = None;
    super::views::settings_panel_frame().show(ui, |ui| {
        ui.set_min_width((ui.available_width() - 1.0).max(0.0));
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Recording Hotkeys")).size(16.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let texture = crate::icons::texture(
                    ui.ctx(),
                    "hotkey-add",
                    include_bytes!("../../assets/icons/hotkey-add.svg"),
                    28,
                );
                let response = ui.add(egui::Button::image_and_text(
                    egui::Image::new((texture.id(), Vec2::splat(14.0))).tint(TEXT),
                    tr("Add Hotkey"),
                ));
                egui::Popup::menu(&response).show(|ui| {
                    for action in CaptureAction::ALL {
                        ui.horizontal(|ui| {
                            action_icon(ui, action);
                            if ui
                                .add_enabled(
                                    settings.capture_bindings().len() < 32,
                                    egui::Button::new(tr(action.label())),
                                )
                                .clicked()
                            {
                                add = Some(action);
                                ui.close();
                            }
                        });
                    }
                });
            });
        });
        ui.add_space(8.0);
        for binding in settings.edit_capture_bindings().iter_mut() {
            ui.separator();
            ui.push_id(binding.id, |ui| {
                ui.horizontal(|ui| {
                    let gap = ui.spacing().item_spacing.x;
                    let label_width =
                        (ui.available_width() - 190.0 - 116.0 - 116.0 - 24.0 - gap * 4.0)
                            .max(155.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(label_width, 42.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_width(label_width);
                            action_icon(ui, binding.action);
                            ui.label(RichText::new(tr(binding.action.label())).strong());
                            if binding.action == CaptureAction::Bookmark {
                                ui.label(
                                    RichText::new(tr("(long recording)"))
                                        .size(11.0)
                                        .color(MUTED),
                                );
                            }
                        },
                    );
                    let listening = capturing == Some(binding.id);
                    let key = if listening {
                        tr("Press keys…")
                    } else if binding.key.is_empty() {
                        tr("Not set")
                    } else {
                        &binding.key
                    };
                    let key_width = 190.0
                        + if binding.action == CaptureAction::Clip {
                            0.0
                        } else {
                            116.0 + gap
                        };
                    if ui
                        .add_sized(
                            [key_width, 38.0],
                            egui::Button::new(
                                RichText::new(crate::key_labels::display(key)).monospace(),
                            )
                            .fill(if listening { ACCENT_DARK } else { BG })
                            .corner_radius(6.0),
                        )
                        .on_hover_text(tr("Click to set shortcut. Escape cancels."))
                        .clicked()
                    {
                        selected = Some(binding.id);
                    }
                    if binding.action == CaptureAction::Clip {
                        super::motion::ComboBox::from_id_salt("duration")
                            .width(116.0)
                            .selected_text(duration_label(binding.clip_seconds))
                            .show_ui(ui, |ui| {
                                for seconds in [5, 10, 15, 30, 45, 60, 90, 120] {
                                    ui.selectable_value(
                                        &mut binding.clip_seconds,
                                        seconds,
                                        duration_label(seconds),
                                    );
                                }
                            });
                    }
                    super::motion::ComboBox::from_id_salt("trigger")
                        .width(116.0)
                        .selected_text(match binding.trigger {
                            HotkeyTrigger::Tap => tr("Tap"),
                            HotkeyTrigger::LongPress => tr("Long press"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut binding.trigger,
                                HotkeyTrigger::Tap,
                                tr("Tap"),
                            );
                            ui.selectable_value(
                                &mut binding.trigger,
                                HotkeyTrigger::LongPress,
                                tr("Long press"),
                            )
                            .on_hover_text(tr("Hold for half a second to activate once."));
                        });
                    super::motion::vertical_menu_button(ui, |ui| {
                        ui.set_min_width(136.0);
                        ui.spacing_mut().button_padding = Vec2::new(12.0, 7.0);
                        ui.spacing_mut().item_spacing.y = 4.0;
                        if ui
                            .add_enabled(
                                !binding.key.is_empty(),
                                egui::Button::new(tr("Unset hotkey")),
                            )
                            .clicked()
                        {
                            binding.key.clear();
                            ui.close();
                        }
                        if removable_actions.contains(&binding.action)
                            && widgets::destructive_button(ui, tr("Remove hotkey"), false).clicked()
                        {
                            remove = Some(binding.id);
                            ui.close();
                        }
                    });
                });
            });
        }
        if settings.capture_bindings().is_empty() {
            ui.label(RichText::new(tr("Add a hotkey to get started.")).color(MUTED));
        }
    });
    if let Some(id) = remove {
        settings.remove_capture_binding(id);
    }
    if let Some(action) = add {
        let id = settings
            .edit_capture_bindings()
            .iter()
            .map(|b| b.id)
            .max()
            .unwrap_or(0)
            + 1;
        let clip_seconds = settings.replay_seconds;
        settings.edit_capture_bindings().push(CaptureHotkey {
            id,
            action,
            clip_seconds,
            ..CaptureHotkey::default()
        });
        selected = Some(id);
    }
    settings.sync_primary_bindings();
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tap_and_hold_can_share_a_key_but_duplicate_triggers_are_rejected() {
        let mut settings = Settings::default();
        let mut bindings = settings.capture_bindings();
        bindings[1].key = bindings[0].key.clone();
        settings.recording_hotkeys = Some(bindings.clone());
        assert!(parse_bindings(&settings).is_err());
        bindings[1].trigger = HotkeyTrigger::LongPress;
        settings.recording_hotkeys = Some(bindings);
        assert_eq!(parse_bindings(&settings).unwrap().len(), 4);
    }
    #[test]
    fn unset_and_removed_bindings_do_not_register_and_empty_list_stays_empty() {
        let mut settings = Settings::default();
        settings.edit_capture_bindings()[0].key.clear();
        assert_eq!(parse_bindings(&settings).unwrap().len(), 3);
        settings.recording_hotkeys = Some(Vec::new());
        assert!(parse_bindings(&settings).unwrap().is_empty());
    }
}
