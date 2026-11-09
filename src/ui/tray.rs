use super::*;
impl Gavnel {
    pub(super) fn create_tray(&mut self) -> Result<TrayIcon> {
        i18n::with_language(self.settings.language, || self.build_tray())
    }

    fn build_tray(&mut self) -> Result<TrayIcon> {
        let menu = Menu::new();
        let show = MenuItem::new(tr("Open Gavnel"), true, None);
        let save = MenuItem::new(tr("Save recent gameplay"), true, None);
        let start = MenuItem::new(tr("Start clipping"), true, None);
        let full = MenuItem::new(tr("Start/stop long recording"), true, None);
        let stop = MenuItem::new(tr("Stop capture / save recording"), true, None);
        let exit = MenuItem::new(tr("Exit"), true, None);
        menu.append_items(&[
            &show,
            &save,
            &PredefinedMenuItem::separator(),
            &start,
            &full,
            &stop,
            &PredefinedMenuItem::separator(),
            &exit,
        ])?;
        self.tray_items = vec![
            (show.clone(), "Open Gavnel"),
            (save.clone(), "Save recent gameplay"),
            (start.clone(), "Start clipping"),
            (full.clone(), "Start/stop long recording"),
            (stop.clone(), "Stop capture / save recording"),
            (exit.clone(), "Exit"),
        ];
        let (show_id, save_id, start_id, stop_id, exit_id) = (
            show.id().clone(),
            save.id().clone(),
            start.id().clone(),
            stop.id().clone(),
            exit.id().clone(),
        );
        let commands = self.engine.commands.clone();
        let full_id = full.id().clone();
        let active = self.active.clone();
        let window_actions = self.window_actions.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                window_actions.show();
            } else if event.id == save_id {
                let _ = commands.try_send(Command::Save);
            } else if event.id == start_id {
                let mut settings = active.lock().unwrap().clone();
                settings.recording_mode = RecordingMode::Clips;
                let _ = commands.try_send(Command::Start(Box::new(settings)));
            } else if event.id == full_id {
                let settings = active.lock().unwrap().clone();
                let _ = commands.try_send(Command::ToggleLongRecording(Box::new(settings)));
            } else if event.id == stop_id {
                let _ = commands.try_send(Command::Stop);
            } else if event.id == exit_id {
                window_actions.exit();
            }
        }));
        let window_actions = self.window_actions.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                window_actions.show();
            }
        }));
        Ok(TrayIconBuilder::new()
            .with_tooltip(tr("Gavnel · Click to open · Right-click to exit"))
            .with_icon(Icon::from_rgba(icon_rgba(), 32, 32)?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?)
    }
    pub(super) fn refresh_tray_language(&self) {
        i18n::with_language(self.settings.language, || {
            for (item, source) in &self.tray_items {
                item.set_text(tr(source));
            }
            if let Some(tray) = &self.tray {
                let _ = tray.set_tooltip(Some(tr("Gavnel · Click to open · Right-click to exit")));
            }
        });
    }
}
