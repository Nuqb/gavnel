use super::*;
impl Gavnel {
    pub(super) fn open_player(&mut self, path: &std::path::Path) {
        if self.saves.replacing(path) {
            self.saves.show_for(path);
            return;
        }
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            if let Err(error) = platform::open(path) {
                self.message = i18n::message(
                    "Could not open screenshot: {error}",
                    &[("error", &format!("{error:#}"))],
                );
            }
            return;
        }
        match crate::player::Player::new(
            path.to_owned(),
            self.parent_window,
            self.context.clone(),
            self.visible.clone(),
        ) {
            Ok(player) => {
                self.player_controls = playback_overlay::Visibility::default();
                self.player = Some(player);
                self.page = Page::Player;
                self.quick_open = false;
            }
            Err(e) => {
                self.message = i18n::message(
                    "Could not open the clip: {error}",
                    &[("error", &format!("{e:#}"))],
                )
            }
        }
    }
    pub(super) fn playback(&mut self, ui: &mut egui::Ui) {
        let mut back = self.playback_shortcuts(ui);
        if !self.player_fullscreen {
            back |= self.playback_header(ui);
            ui.add_space(16.0);
        }
        if back {
            fullscreen(ui.ctx(), &mut self.player_fullscreen, false);
            self.player = None;
            self.page = Page::Clips;
            return;
        }
        if self.page != Page::Player {
            return;
        }
        self.playback_surface(ui);
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }

    fn playback_shortcuts(&mut self, ui: &mut egui::Ui) -> bool {
        if !self.quick_open
            && self.saves.modal.is_none()
            && exit_fullscreen_on_escape(ui, &mut self.player_fullscreen)
        {
            return false;
        }
        if self.quick_open || self.saves.modal.is_some() || ui.ctx().egui_wants_keyboard_input() {
            return false;
        }
        let keys = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::Space),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                input.consume_key(egui::Modifiers::NONE, egui::Key::F)
                    | input.consume_key(egui::Modifiers::NONE, egui::Key::F11),
            )
        });
        if let Some(player) = &self.player {
            if keys.0
                && let Err(error) = player.toggle()
            {
                self.message = error.to_string();
            }
            if keys.1 {
                let _ = player.seek(player.position() - 5.0);
            }
            if keys.2 {
                let _ = player.seek(player.position() + 5.0);
            }
        }
        fullscreen_keys(ui.ctx(), &mut self.player_fullscreen, keys.3, keys.4)
    }

    fn playback_header(&mut self, ui: &mut egui::Ui) -> bool {
        let mut back = false;
        let mut edit = None;
        ui.horizontal(|ui| {
            back |= super::back_button::show(ui, "Library").clicked();
            if let Some(player) = &self.player {
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            player
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                        )
                        .strong(),
                    )
                    .truncate(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if action_icon(
                        ui,
                        "edit",
                        include_bytes!("../../assets/icons/edit.svg"),
                        "Edit",
                    )
                    .clicked()
                    {
                        edit = Some(player.path.clone());
                    }
                    if action_icon(
                        ui,
                        "folder",
                        include_bytes!("../../assets/icons/folder.svg"),
                        "Show in folder",
                    )
                    .clicked()
                    {
                        let _ = platform::reveal(&player.path);
                    }
                    if action_icon(
                        ui,
                        "open-external",
                        include_bytes!("../../assets/icons/open-external.svg"),
                        "Open externally",
                    )
                    .clicked()
                    {
                        let _ = player.pause();
                        let _ = platform::open(&player.path);
                    }
                });
            }
        });
        if let Some(path) = edit {
            self.begin_editor(vec![path]);
        }
        back
    }

    fn playback_surface(&mut self, ui: &mut egui::Ui) {
        self.take_video_click(ui.ctx());
        let Some(player) = &mut self.player else {
            return;
        };
        let size = Vec2::new(ui.available_width(), ui.available_height().max(160.0));
        let (surface, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect_filled(surface, 8.0, Color32::BLACK);
        let (video, controls) = surface_rects(surface, &player.bookmarks);
        let error = player.error();
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::unique("video-layout-rect"), video));
        if let Err(error) = player.layout(
            video,
            ui.ctx().pixels_per_point(),
            error.is_none() && self.saves.modal.is_none(),
        ) {
            self.message = error.to_string();
        }
        let popup = ui
            .ctx()
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("video-popup-rect")));
        player.set_occlusion(video, ui.ctx().pixels_per_point(), popup);
        if let Some(error) = error {
            ui.painter().text(
                video.center(),
                egui::Align2::CENTER_CENTER,
                error,
                egui::FontId::proportional(13.0),
                MUTED,
            );
        }
        let mut state = playback_controls::State {
            duration: player.duration(),
            position: player
                .seek_preview
                .unwrap_or_else(|| player.position())
                .min(player.duration()),
            volume: player.volume,
            muted: player.muted,
            paused: player.paused(),
            ended: player.ended(),
            fullscreen: self.player_fullscreen,
            bookmarks: &player.bookmarks,
        };
        let pointer =
            crate::player::overlay::pointer(self.parent_window, ui.ctx().pixels_per_point())
                .filter(|point| surface.contains(*point));
        let visible = self.player_controls.visible(
            ui.input(|input| input.time),
            pointer,
            controls,
            state.paused || state.ended,
        ) && player.ready()
            && self.saves.modal.is_none()
            && !self.quick_open
            && !self.clips_open
            && !self.long_recording_menu_open;
        let action =
            playback_overlay::show(ui.ctx(), self.parent_window, controls, &mut state, visible);
        let audio = (state.volume, state.muted);
        apply_controls(player, &action, audio, &mut self.message);
        if action.back {
            self.player = None;
            self.page = Page::Clips;
            return;
        }
        if action.close_requested {
            self.window_actions.exit();
        }
        if let Some(error) = action.error {
            self.message = error;
        }
        if action.toggle_fullscreen {
            let enabled = !self.player_fullscreen;
            fullscreen(ui.ctx(), &mut self.player_fullscreen, enabled);
        }
    }

    fn take_video_click(&mut self, ctx: &egui::Context) {
        if self
            .player
            .as_ref()
            .is_none_or(|player| !player.take_click())
        {
            return;
        }
        if let Some(player) = &self.player {
            let _ = player.toggle();
        }
        self.quick_open = false;
        self.clips_open = false;
        self.long_recording_menu_open = false;
        if self.clips_capture_owned {
            self.capturing_hotkey.store(false, Ordering::Relaxed);
            self.clips_capture_owned = false;
        }
        ctx.data_mut(|data| data.remove::<egui::Rect>(egui::Id::unique("video-popup-rect")));
    }
}

pub(super) fn fullscreen(ctx: &egui::Context, current: &mut bool, enabled: bool) {
    if *current != enabled {
        *current = enabled;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(enabled));
    }
}

fn exit_fullscreen_on_escape(ui: &mut egui::Ui, current: &mut bool) -> bool {
    if *current && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        fullscreen(ui.ctx(), current, false);
        return true;
    }
    false
}

fn fullscreen_keys(ctx: &egui::Context, current: &mut bool, escape: bool, toggle: bool) -> bool {
    let leave_fullscreen = escape && *current;
    if leave_fullscreen || toggle {
        let enabled = !*current && !leave_fullscreen;
        fullscreen(ctx, current, enabled);
    }
    escape && !leave_fullscreen
}

fn apply_controls(
    player: &mut crate::player::Player,
    action: &playback_controls::Action,
    audio: (f64, bool),
    message: &mut String,
) {
    if action.toggle_play
        && let Err(error) = player.toggle()
    {
        *message = error.to_string();
    }
    if action.audio_changed {
        (player.volume, player.muted) = audio;
        if let Err(error) = player.audio() {
            *message = error.to_string();
        }
    }
    if let Some(position) = action.preview {
        player.seek_preview = Some(position);
    }
    if let Some(position) = action.seek {
        player.seek_preview = None;
        if let Err(error) = player.seek(position) {
            *message = error.to_string();
        }
    }
}

fn surface_rects(surface: egui::Rect, bookmarks: &[f64]) -> (egui::Rect, egui::Rect) {
    let top = (surface.bottom() - playback_controls::height(bookmarks)).max(surface.top());
    (
        surface,
        egui::Rect::from_min_max(egui::pos2(surface.left(), top), surface.max),
    )
}

pub(super) fn time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

fn action_icon(ui: &mut egui::Ui, name: &'static str, svg: &[u8], label: &str) -> egui::Response {
    let texture = crate::icons::texture(ui.ctx(), name, svg, 32);
    let response = ui.add(
        egui::Button::image(egui::Image::new((texture.id(), Vec2::splat(20.0))).tint(TEXT))
            .frame_when_inactive(false)
            .stroke(egui::Stroke::NONE),
    );
    let mut tooltip = egui::Tooltip::for_enabled(&response);
    tooltip.popup = tooltip
        .popup
        .align(egui::emath::RectAlign::TOP_END)
        .align_alternatives(&[]);
    tooltip.show(|ui| {
        ui.label(tr(label));
    });
    response
}

#[cfg(test)]
#[path = "playback_tests.rs"]
mod tests;
