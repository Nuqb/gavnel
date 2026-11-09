use super::*;
impl Gavnel {
    pub(super) fn create_tray(&self) -> Result<TrayIcon> {
        let menu = Menu::new();
        let show = MenuItem::new("Open Gavnel", true, None);
        let save = MenuItem::new("Save recent gameplay", true, None);
        let start = MenuItem::new("Start clipping", true, None);
        let full = MenuItem::new("Start/stop long recording", true, None);
        let stop = MenuItem::new("Stop capture / save recording", true, None);
        let exit = MenuItem::new("Exit", true, None);
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
            .with_tooltip("Gavnel · Click to open · Right-click to exit")
            .with_icon(Icon::from_rgba(icon_rgba(), 32, 32)?)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?)
    }
}
