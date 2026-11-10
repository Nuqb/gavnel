mod add_game;
mod audio_apps;
mod capture_view;
mod checkbox;
mod clip_card;
mod clips_menu;
mod delete;
mod editor;
use crate::typography as fonts;
mod highlights;
mod language;
mod library_view;
mod microphone_test;
mod motion;
pub(crate) mod onboarding;
mod playback;
mod preferences;
mod quick;
mod radio;
mod settings;
mod settings_audio;
mod settings_games;
mod settings_general;
mod settings_recording;
mod settings_storage;
mod shortcuts;
mod source_card;
mod source_picker;
#[cfg(test)]
mod test_render;
mod theme;
mod toolbar;
mod tray;
mod updates;
mod views;
mod widgets;

use crate::i18n::{self, tr};
use crate::keyboard::Manager as GlobalHotKeyManager;
use crate::{
    audio::{self, AudioDevice},
    config::{CaptureMode, LibrarySort as Sort, RecordingMode, Settings},
    engine::{Command, Engine, Notify, Status},
    library::Library,
    platform,
    sources::{self, MonitorInfo, WindowInfo},
    thumbnails::Thumbnails,
};
use anyhow::{Context, Result};
use eframe::egui::{self, Color32, RichText, Vec2};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState, hotkey::HotKey};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const ACCENT: Color32 = Color32::from_rgb(255, 132, 49);
const ACCENT_DARK: Color32 = Color32::from_rgb(64, 38, 23);
const TEXT: Color32 = Color32::from_rgb(240, 240, 242);
const MUTED: Color32 = Color32::from_rgb(148, 148, 156);
const BG: Color32 = Color32::from_rgb(17, 17, 19);
const PANEL: Color32 = Color32::from_rgb(24, 24, 27);
const CARD: Color32 = Color32::from_rgb(31, 31, 35);
const LINE: Color32 = Color32::from_rgb(45, 45, 50);
const DANGER: Color32 = Color32::from_rgb(242, 94, 104);
const DANGER_DARK: Color32 = Color32::from_rgb(58, 29, 34);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Editor,
    Player,
    Clips,
    Capture,
    Settings,
}
#[derive(Clone, Copy, PartialEq)]
enum SettingsPage {
    Recording,
    Audio,
    Storage,
    General,
    Games,
}
#[derive(Clone, Copy, PartialEq)]
enum QuickPage {
    Overview,
    Quality,
    Audio,
}
pub struct Gavnel {
    onboarding: Option<onboarding::Wizard>,
    updates: crate::update::Controller,
    lifecycle_test: bool,
    context: egui::Context,
    player: Option<crate::player::Player>,
    editor: Option<editor::Editor>,
    media: crate::media::MediaService,
    montage_selection: Vec<PathBuf>,
    selecting_clips: bool,
    parent_window: windows::Win32::Foundation::HWND,
    window_actions: platform::WindowActions,
    engine: Engine,
    settings: Settings,
    active: Arc<Mutex<Settings>>,
    page: Page,
    settings_page: SettingsPage,
    hotkeys: GlobalHotKeyManager,
    hotkey: Arc<Mutex<Vec<crate::capture_hotkeys::RegisteredShortcut>>>,
    pressed_hotkeys: Arc<Mutex<crate::capture_hotkeys::PressedShortcuts>>,
    tray: Option<TrayIcon>,
    tray_items: Vec<(MenuItem, &'static str)>,
    visible: Arc<AtomicBool>,
    quitting: Arc<AtomicBool>,
    monitors: Vec<MonitorInfo>,
    windows: Vec<WindowInfo>,
    inputs: Vec<AudioDevice>,
    outputs: Vec<AudioDevice>,
    mic_test: Option<audio::MicTest>,
    capturing_ptt: bool,
    settings_edited: Option<Instant>,
    source_picker: Option<source_picker::SourcePicker>,
    library: Library,
    thumbnails: Thumbnails,
    textures: HashMap<PathBuf, egui::TextureHandle>,
    message: String,
    storage_text: String,
    add_game_dialog: Option<add_game::Dialog>,
    delete_pending: Option<PathBuf>,
    delete_error: Option<String>,
    last_saved: u64,
    last_library: Instant,
    search: String,
    game_filter: String,
    quick_page: QuickPage,
    quick_open: bool,
    clips_open: bool,
    long_recording_menu_open: bool,
    clips_capture_owned: bool,
    _activation: platform::WindowActivator,
    started: Instant,
    logo_cycle: Option<Instant>,
    capturing_hotkey: Arc<AtomicBool>,
    capturing_action: u64,
}
pub fn icon_rgba() -> Vec<u8> {
    crate::branding::icon_rgba(32)
}
impl Gavnel {
    pub fn verify_lifecycle(&self) -> std::thread::JoinHandle<Result<()>> {
        self.window_actions.verify_lifecycle()
    }
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        background: bool,
        lifecycle_test: bool,
    ) -> Result<Self> {
        let ctx = cc.egui_ctx.clone();
        theme::apply(&ctx);
        let (mut settings, load_message) = Settings::load_preserving_storage();
        if lifecycle_test {
            settings.close_to_tray = true;
        }
        let visible = Arc::new(AtomicBool::new(!background));
        let quitting = Arc::new(AtomicBool::new(false));
        let parent_window = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match cc.window_handle()?.as_raw() {
                RawWindowHandle::Win32(h) => windows::Win32::Foundation::HWND(h.hwnd.get() as _),
                _ => anyhow::bail!("Gavnel requires a Windows desktop window"),
            }
        };
        let window_actions = platform::WindowActions::new(
            parent_window,
            ctx.clone(),
            visible.clone(),
            quitting.clone(),
        );
        let activation = platform::WindowActivator::new(window_actions.clone())?;
        let engine = Engine::new(Notify {
            ctx: ctx.clone(),
            visible: visible.clone(),
        });
        engine.send(Command::SetFeedback(
            settings.clip_feedback.clone(),
            settings.language,
        ))?;
        let active = Arc::new(Mutex::new(settings.clone()));
        let hotkeys = GlobalHotKeyManager::new()?;
        let library = Library::new(ctx.clone(), visible.clone());
        let thumbnails = Thumbnails::new(ctx.clone(), visible.clone());
        let mut app = Self {
            onboarding: None,
            updates: crate::update::Controller::default(),
            lifecycle_test,
            context: ctx.clone(),
            player: None,
            editor: None,
            media: crate::media::MediaService::new(),
            montage_selection: Vec::new(),
            selecting_clips: false,
            parent_window,
            window_actions,
            engine,
            active,
            storage_text: settings.clips_dir.to_string_lossy().into_owned(),
            settings,
            page: Page::Clips,
            settings_page: SettingsPage::Recording,
            hotkeys,
            hotkey: Arc::new(Mutex::new(Vec::new())),
            pressed_hotkeys: Arc::new(Mutex::new(Default::default())),
            tray: None,
            tray_items: Vec::new(),
            visible,
            quitting,
            monitors: sources::monitors(),
            windows: sources::windows(),
            inputs: audio::devices(true),
            outputs: audio::devices(false),
            mic_test: None,
            capturing_ptt: false,
            settings_edited: None,
            source_picker: None,
            library,
            thumbnails,
            textures: HashMap::new(),
            message: load_message,
            add_game_dialog: None,
            delete_pending: None,
            delete_error: None,
            last_saved: 0,
            last_library: Instant::now(),
            search: String::new(),
            game_filter: String::new(),
            quick_page: QuickPage::Overview,
            quick_open: false,
            clips_open: false,
            long_recording_menu_open: false,
            clips_capture_owned: false,
            _activation: activation,
            started: Instant::now(),
            logo_cycle: None,
            capturing_hotkey: Arc::new(AtomicBool::new(false)),
            capturing_action: 1,
        };
        if let Err(e) = app.bind_hotkey() {
            app.message = e.to_string();
        }
        app.install_shortcut_handler();
        app.tray = match app.create_tray() {
            Ok(t) => Some(t),
            Err(e) => {
                app.message = format!("Tray icon unavailable; closing the window will exit: {e}");
                None
            }
        };
        app.library.refresh(&app.settings.clips_dir);
        if !lifecycle_test && !app.settings.onboarding_completed {
            app.begin_onboarding();
            app.window_actions.show();
        } else if !lifecycle_test && app.settings.start_on_launch(background) {
            app.engine
                .send(Command::Start(Box::new(app.settings.clone())))?;
        }
        Ok(app)
    }
    fn bind_hotkey(&mut self) -> Result<()> {
        self.bind_shortcuts()
    }

    fn send(&mut self, command: Command) {
        if let Err(e) = self.engine.send(command) {
            self.message = e.to_string();
        }
    }
    fn toggle_recording(&mut self, status: &Status) {
        if self.settings.recording_mode == RecordingMode::Off {
            self.page = Page::Settings;
            self.settings_page = SettingsPage::Recording;
            self.message = tr("Choose a recording mode to enable capture.").into();
            return;
        }
        if status.armed {
            self.send(Command::Stop);
        } else {
            match self.apply() {
                Ok(()) => self.send(Command::Start(Box::new(self.settings.clone()))),
                Err(e) => self.message = format!("{e:#}"),
            }
        }
    }
    fn recording_button(&self, status: &Status) -> &'static str {
        if status.armed {
            if status.full_recording {
                tr("Stop & save")
            } else {
                tr("Stop capture")
            }
        } else if self.settings.recording_mode == RecordingMode::Off {
            tr("Recording off")
        } else if self.settings.recording_mode.automatic() {
            tr("Enable auto recording")
        } else if self.settings.recording_mode.full() {
            tr("Start recording")
        } else {
            tr("Start clipping")
        }
    }
    fn recording_mode_controls(&mut self, ui: &mut egui::Ui, _status: &Status) {
        let previous = self.settings.recording_mode;
        if widgets::recording_modes(ui, &mut self.settings.recording_mode) {
            let previous_source = self.settings.mode;
            if self.settings.recording_mode.automatic() {
                self.settings.mode = CaptureMode::Automatic;
            }
            if let Err(error) = self.apply() {
                self.settings.recording_mode = previous;
                self.settings.mode = previous_source;
                self.message = error.to_string();
            }
        }
    }

    fn capture_hotkey(&mut self, ctx: &egui::Context) {
        if !self.capturing_hotkey.load(Ordering::Relaxed) {
            return;
        }
        let keys = ctx.input(|i| i.events.clone());
        for event in keys {
            if let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            {
                self.capturing_hotkey.store(false, Ordering::Relaxed);
                if key == egui::Key::Escape {
                    return;
                }
                let mut parts = Vec::new();
                if modifiers.ctrl {
                    parts.push("Control".to_owned());
                }
                if modifiers.alt {
                    parts.push("Alt".to_owned());
                }
                if modifiers.shift {
                    parts.push("Shift".to_owned());
                }
                parts.push(key.name().to_owned());
                let candidate = parts.join("+");
                if candidate.parse::<HotKey>().is_ok() {
                    match self.select_shortcut(candidate) {
                        Ok(()) => self.message = tr("Shortcut selected.").into(),
                        Err(error) => self.message = error.to_string(),
                    }
                } else {
                    self.message=tr("That key cannot be used as a global shortcut. Try a function key or key combination.").into();
                }
                return;
            }
        }
    }
    fn logo_phase(&self) -> Option<f32> {
        let elapsed = self.started.elapsed().as_secs_f32();
        if elapsed < 1.8 {
            Some(elapsed / 1.8)
        } else {
            self.logo_cycle
                .map(|start| (start.elapsed().as_secs_f32() / 1.8).min(1.0))
        }
    }
}
impl eframe::App for Gavnel {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        self.updates
            .poll(ctx, !self.lifecycle_test && self.updates.installed);
        i18n::with_language(self.settings.language, || {
            if self.quitting.load(Ordering::Acquire) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            if ctx.input(|i| i.viewport().close_requested())
                && !self.quitting.load(Ordering::Relaxed)
                && self.tray.is_some()
                && self.settings.close_to_tray
                && self.onboarding.is_none()
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.settings != *self.active.lock().unwrap()
                    && let Err(error) = self.apply()
                {
                    self.message = error.to_string();
                }
                self.window_actions.hide();
                if let Some(player) = &self.player {
                    let _ = player.pause();
                }
                return;
            }
            if !self.visible.load(Ordering::Relaxed) && !self.quitting.load(Ordering::Relaxed) {
                return;
            }
            if self.onboarding.is_some() {
                self.show_onboarding(root);
                return;
            }
            fonts::ensure_language(ctx, self.settings.language.code());
            let status = self.engine.snapshot();
            if let Some(notice) = self.library.notice.lock().unwrap().take() {
                self.message = notice;
                self.textures.clear();
            }
            if self.started.elapsed().as_secs_f32() >= 1.8 {
                if self
                    .logo_cycle
                    .is_some_and(|start| start.elapsed().as_secs_f32() >= 1.8)
                {
                    self.logo_cycle = None;
                }
                if self.logo_cycle.is_none() && status.saving > 0 {
                    self.logo_cycle = Some(Instant::now());
                }
            }
            self.capture_hotkey(ctx);
            if self.logo_phase().is_some() {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            if status.saved_count != self.last_saved {
                self.last_saved = status.saved_count;
                self.library.refresh(&self.settings.clips_dir);
            }
            if self.page == Page::Clips && self.last_library.elapsed() > Duration::from_secs(5) {
                self.last_library = Instant::now();
                self.library.refresh(&self.settings.clips_dir);
            }
            i18n::with_language(self.settings.language, || {
                self.draw(root, &status);
                self.show_source_picker(ctx, &status);
                if let Some(dialog) = &mut self.add_game_dialog
                    && dialog.show(ctx, &mut self.settings)
                {
                    self.add_game_dialog = None;
                }
            });
            let changed = self.settings != *self.active.lock().unwrap();
            if changed {
                let edited = self.settings_edited.get_or_insert_with(Instant::now);
                if ctx.input(|input| {
                    input.pointer.any_down()
                        || input.events.iter().any(|event| {
                            matches!(
                                event,
                                egui::Event::Text(_) | egui::Event::Key { pressed: true, .. }
                            )
                        })
                }) {
                    *edited = Instant::now();
                } else if edited.elapsed() >= Duration::from_millis(400) {
                    if let Err(error) = self.apply() {
                        self.message = error.to_string();
                    }
                    self.settings_edited = None;
                }
                ctx.request_repaint_after(Duration::from_millis(100));
            } else {
                self.settings_edited = None;
            }
            motion::finish(ctx);
            if self.page != Page::Player && self.page != Page::Editor {
                self.player = None;
            }
            if let Some(path) = self.delete_pending.clone() {
                let clip = self
                    .library
                    .clips
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|clip| clip.path == path)
                    .cloned();
                match delete::show(ctx, &path, clip.as_ref(), self.delete_error.as_deref()) {
                delete::Action::Cancel => { self.delete_pending = None; self.delete_error = None; },
                delete::Action::Delete => match crate::library::delete_capture(&path) {
                    Ok(()) => {
                        self.textures.remove(&path);
                        self.library.refresh(&self.settings.clips_dir);
                        self.message = tr("Clip deleted.").into();
                        self.delete_pending = None;
                        self.delete_error = None;
                    },
                    Err(_) => self.delete_error = Some(tr("Could not delete this clip. It may have moved or be open in another player. Close this dialog and refresh the Library, then try again.").into()),
                },
                delete::Action::None => {},
            }
            }
        });
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.lifecycle_test {
            return;
        }
        // Save before the engine shuts down; shutdown itself must not turn the
        // next launch off or restart capture to apply pending preferences.
        self.settings.capture_enabled = Some(self.engine.snapshot().armed);
        if let Err(error) = self.settings.save() {
            eprintln!("Could not save preferences on exit: {error:#}");
            let mut saved = self.active.lock().unwrap().clone();
            saved.capture_enabled = self.settings.capture_enabled;
            if let Err(error) = saved.save() {
                eprintln!("Could not save capture state on exit: {error:#}");
            }
        }
    }
}
fn recording_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}
