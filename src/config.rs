use anyhow::{Context, Result, bail};
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureMode {
    Monitor,
    Window,
    #[default]
    Automatic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibrarySort {
    #[default]
    Newest,
    Oldest,
    Largest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingMode {
    #[default]
    Clips,
    FullRecording,
    AutomaticSessions,
    Off,
}
impl RecordingMode {
    pub fn preference(self) -> Self {
        if self == Self::FullRecording {
            Self::Clips
        } else {
            self
        }
    }
    pub fn full(self) -> bool {
        matches!(self, Self::FullRecording | Self::AutomaticSessions)
    }
    pub fn automatic(self) -> bool {
        self == Self::AutomaticSessions
    }
    pub fn start_on_launch(self, background: bool) -> bool {
        self.automatic() || (background && self == Self::Clips)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MicChannel {
    #[default]
    Left,
    Right,
    Mix,
}
impl MicChannel {
    pub fn mono(self, sample: [f32; 2]) -> [f32; 2] {
        let value = match self {
            Self::Left => sample[0],
            Self::Right => sample[1],
            Self::Mix => (sample[0] + sample[1]) * 0.5,
        };
        [value; 2]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameProfile {
    pub name: String,
    pub executable: String,
}

mod game_catalog;
use game_catalog::OFFICIAL_GAMES;

impl GameProfile {
    pub fn is_official(&self) -> bool {
        game_catalog::official_name(&self.executable).is_some()
    }

    fn official_games() -> impl Iterator<Item = Self> {
        OFFICIAL_GAMES.iter().map(|&(name, executable)| Self {
            name: name.into(),
            executable: executable.into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipFeedback {
    pub sound: bool,
    pub notification: bool,
    pub volume: f32,
}
impl Default for ClipFeedback {
    fn default() -> Self {
        Self {
            sound: true,
            notification: true,
            volume: 0.6,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseSuppression {
    #[default]
    None,
    Standard,
    Neural,
}
impl NoiseSuppression {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Standard => "Standard",
            Self::Neural => "AI (RNNoise)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub onboarding_completed: bool,
    pub check_for_updates: bool,
    pub language: crate::i18n::Language,
    /// None preserves startup behavior for preferences saved before this field existed.
    pub capture_enabled: Option<bool>,
    pub library_compact: bool,
    pub library_group_by_game: bool,
    pub library_sort: LibrarySort,
    pub version: u32,
    pub mode: CaptureMode,
    pub recording_mode: RecordingMode,
    pub monitor_index: usize,
    pub window_title: String,
    pub window_executable: String,
    pub height: u32,
    pub fps: u32,
    pub bitrate_mbps: u32,
    pub replay_seconds: u32,
    pub desktop_audio: bool,
    pub microphone: bool,
    pub input_device: String,
    pub desktop_volume: f32,
    pub mic_volume: f32,
    pub mic_mono: bool,
    pub mic_channel: MicChannel,
    pub separate_audio_tracks: bool,
    // Retain the legacy keys so existing sensitivity preferences migrate unchanged.
    #[serde(rename = "noise_suppression")]
    pub input_sensitivity_enabled: bool,
    #[serde(rename = "noise_threshold_db")]
    pub input_sensitivity_db: f32,
    pub noise_suppression_mode: NoiseSuppression,
    pub push_to_talk: bool,
    pub push_to_talk_key: u16,
    pub output_devices: Option<Vec<String>>,
    pub hotkey: String,
    pub long_recording_hotkey: String,
    pub bookmark_hotkey: String,
    pub recording_hotkeys: Option<Vec<crate::capture_hotkeys::CaptureHotkey>>,
    pub clip_feedback: ClipFeedback,
    pub clips_dir: PathBuf,
    pub storage_limit_gb: Option<u32>,
    pub start_with_windows: bool,
    pub close_to_tray: bool,
    pub profiles: Vec<GameProfile>,
}

impl Default for Settings {
    fn default() -> Self {
        let base = UserDirs::new()
            .and_then(|u| u.video_dir().map(|p| p.to_owned()))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            onboarding_completed: false,
            check_for_updates: true,
            language: crate::i18n::Language::default(),
            capture_enabled: None,
            library_compact: false,
            library_group_by_game: false,
            library_sort: LibrarySort::Newest,
            version: 1,
            mode: CaptureMode::Automatic,
            recording_mode: RecordingMode::Clips,
            monitor_index: 1,
            window_title: String::new(),
            window_executable: String::new(),
            height: 1080,
            fps: 60,
            bitrate_mbps: 20,
            replay_seconds: 60,
            desktop_audio: true,
            microphone: false,
            input_device: String::new(),
            desktop_volume: 1.0,
            mic_volume: 1.0,
            mic_mono: false,
            mic_channel: MicChannel::Left,
            separate_audio_tracks: false,
            input_sensitivity_enabled: false,
            input_sensitivity_db: -75.0,
            noise_suppression_mode: NoiseSuppression::None,
            push_to_talk: false,
            push_to_talk_key: 0x56,
            output_devices: None,
            hotkey: "F8".into(),
            long_recording_hotkey: "Alt+F7".into(),
            bookmark_hotkey: "F4".into(),
            recording_hotkeys: None,
            clip_feedback: ClipFeedback::default(),
            clips_dir: base.join("Gavnel"),
            storage_limit_gb: None,
            start_with_windows: false,
            close_to_tray: false,
            profiles: GameProfile::official_games().collect(),
        }
    }
}

impl Settings {
    pub fn add_game_profile(&mut self, name: &str, executable: &str) -> Result<()> {
        let name = name.trim();
        let executable = executable.trim();
        anyhow::ensure!(
            executable.to_ascii_lowercase().ends_with(".exe") && !executable.contains(['\\', '/']),
            "Enter an executable filename, such as GameClient.exe."
        );
        anyhow::ensure!(!name.is_empty(), "Enter a name for this game.");
        anyhow::ensure!(
            !self
                .profiles
                .iter()
                .any(|profile| profile.executable.eq_ignore_ascii_case(executable)),
            "This game is already supported."
        );
        self.profiles.push(GameProfile {
            name: name.into(),
            executable: executable.into(),
        });
        Ok(())
    }

    pub fn start_on_launch(&self, background: bool) -> bool {
        self.recording_mode != RecordingMode::Off
            && self
                .capture_enabled
                .unwrap_or_else(|| self.recording_mode.start_on_launch(background))
    }

    fn restore_shortcut_defaults(&mut self, missing_long: bool, missing_bookmark: bool) {
        use global_hotkey::hotkey::HotKey;
        let same = |a: &str, b: &str| {
            a.parse::<HotKey>()
                .ok()
                .zip(b.parse::<HotKey>().ok())
                .is_some_and(|(a, b)| a == b)
        };
        if missing_long && same(&self.long_recording_hotkey, &self.hotkey) {
            self.long_recording_hotkey = "Alt+F9".into();
        }
        if missing_bookmark {
            for candidate in ["F4", "F6", "F9"] {
                if !same(candidate, &self.hotkey) && !same(candidate, &self.long_recording_hotkey) {
                    self.bookmark_hotkey = candidate.into();
                    break;
                }
            }
        }
    }
    fn restore_official_games(&mut self) {
        let custom = std::mem::take(&mut self.profiles)
            .into_iter()
            .filter(|profile| {
                !profile.is_official()
                    && !game_catalog::retired_builtin(&profile.name, &profile.executable)
            });
        self.profiles = GameProfile::official_games().chain(custom).collect();
    }

    pub fn recording_changed(&self, previous: &Self) -> bool {
        self.mode != previous.mode
            || self.recording_mode != previous.recording_mode
            || self.monitor_index != previous.monitor_index
            || self.window_title != previous.window_title
            || self.window_executable != previous.window_executable
            || self.height != previous.height
            || self.fps != previous.fps
            || self.bitrate_mbps != previous.bitrate_mbps
            || self.desktop_audio != previous.desktop_audio
            || self.microphone != previous.microphone
            || self.input_device != previous.input_device
            || self.desktop_volume != previous.desktop_volume
            || self.mic_volume != previous.mic_volume
            || self.mic_mono != previous.mic_mono
            || self.mic_channel != previous.mic_channel
            || self.profiles != previous.profiles
            || self.separate_audio_tracks != previous.separate_audio_tracks
            || self.noise_suppression_mode != previous.noise_suppression_mode
            || self.input_sensitivity_enabled != previous.input_sensitivity_enabled
            || self.input_sensitivity_db != previous.input_sensitivity_db
            || self.push_to_talk != previous.push_to_talk
            || self.push_to_talk_key != previous.push_to_talk_key
            || self.output_devices != previous.output_devices
            || self.storage_limit_gb != previous.storage_limit_gb
    }
    pub fn path() -> PathBuf {
        ProjectDirs::from("", "Gavnel", "Gavnel")
            .map(|d| d.config_dir().join("settings.json"))
            .unwrap_or_else(|| PathBuf::from("gavnel-settings.json"))
    }
    pub fn validate(&self) -> Result<()> {
        if let Some(bindings) = &self.recording_hotkeys {
            // Leave room to restore required action rows in an older 32-row setup.
            anyhow::ensure!(bindings.len() <= 36, "Too many recording hotkeys.");
            let mut ids = std::collections::HashSet::new();
            for binding in bindings {
                anyhow::ensure!(
                    ids.insert(binding.id) && (5..=120).contains(&binding.clip_seconds),
                    "Invalid recording hotkey settings."
                );
            }
        }
        if self.storage_limit_gb == Some(0) {
            bail!("Choose a storage limit greater than zero.");
        }
        if !self.input_sensitivity_db.is_finite()
            || !(-100.0..=-25.0).contains(&self.input_sensitivity_db)
            || !(1..=254).contains(&self.push_to_talk_key)
        {
            bail!("Invalid microphone settings.");
        }
        if self.version != 1 {
            bail!("This settings file was created by an unsupported Gavnel version.");
        }
        if ![720, 1080, 1440].contains(&self.height) || ![30, 60, 120].contains(&self.fps) {
            bail!("Choose 720p, 1080p or 1440p and 30, 60 or 120 FPS.");
        }
        if !(5..=50).contains(&self.bitrate_mbps) || !(5..=120).contains(&self.replay_seconds) {
            bail!("Bitrate must be 5–50 Mbps and replay length 5–120 seconds.");
        }
        if !self.desktop_volume.is_finite()
            || !self.mic_volume.is_finite()
            || !(0.0..=2.0).contains(&self.desktop_volume)
            || !(0.0..=2.0).contains(&self.mic_volume)
        {
            bail!("Invalid audio level.");
        }
        if self.clips_dir.as_os_str().is_empty() {
            bail!("Choose a clip folder.");
        }
        if !self.clip_feedback.volume.is_finite()
            || !(0.0..=1.0).contains(&self.clip_feedback.volume)
        {
            bail!("Clip sound volume must be between 0 and 100%.");
        }
        Ok(())
    }
    fn load_from(path: &Path) -> Result<Self> {
        let mut value = read_preferences(&path.with_file_name("preferences.json"));
        if path.exists() {
            merge_preferences(&mut value, serde_json::from_slice(&std::fs::read(path)?)?);
        }
        // Older installations have already been configured; only genuinely new
        // profiles enter setup automatically. An explicit false survives exit.
        if value.get("onboarding_completed").is_none()
            && value.as_object().is_some_and(|fields| !fields.is_empty())
        {
            value["onboarding_completed"] = serde_json::json!(true);
        }
        let missing_long = value.get("long_recording_hotkey").is_none();
        let missing_bookmark = value.get("bookmark_hotkey").is_none();
        let mut settings: Self = serde_json::from_value(value)
            .context("Could not read settings; the existing file has been preserved")?;
        settings.restore_official_games();
        settings.recording_mode = settings.recording_mode.preference();
        settings.restore_shortcut_defaults(missing_long, missing_bookmark);
        settings.ensure_capture_action_rows();
        settings.sync_primary_bindings();
        if let Some(folder) = stored_folder(path) {
            settings.clips_dir = folder;
        }
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
    }
    fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        // Storage is version-independent and can be recovered even if other settings fail to load.
        write_json(
            &path.with_file_name("storage.json"),
            &serde_json::json!({ "clips_dir": self.clips_dir }),
        )?;
        if path.exists() {
            std::fs::copy(path, path.with_file_name("settings.previous.json"))
                .context("Could not back up the previous settings")?;
        }
        let mut value = read_preferences(&path.with_file_name("preferences.json"));
        merge_preferences(&mut value, read_preferences(path));
        merge_preferences(&mut value, serde_json::to_value(self)?);
        write_json(path, &value)?;
        write_json(&path.with_file_name("preferences.json"), &value)?;
        Ok(())
    }
    pub fn load_preserving_storage() -> (Self, String) {
        Self::recover_from(&Self::path())
    }
    fn recover_from(path: &Path) -> (Self, String) {
        match Self::load_from(path) {
            Ok(mut settings) => {
                if let Some(folder) = stored_folder(path) {
                    settings.clips_dir = folder;
                }
                (settings, String::new())
            }
            Err(error) => {
                let folder = stored_folder(path)
                    .or_else(|| stored_folder(&path.with_file_name("settings.previous.json")));
                let recovered = folder.is_some();
                // Recover compatible fields individually instead of resetting every preference
                // when a newer version introduces one unfamiliar value.
                let mut values = serde_json::to_value(Self::default()).unwrap();
                values["onboarding_completed"] = serde_json::json!(true);
                let mut has_long_shortcut = false;
                let mut has_bookmark_shortcut = false;
                for source in [
                    path.with_file_name("settings.previous.json"),
                    path.with_file_name("preferences.json"),
                    path.to_owned(),
                ] {
                    if let Some(fields) = read_preferences(&source).as_object() {
                        for (key, value) in fields {
                            if key == "version" {
                                continue;
                            }
                            let mut candidate = values.clone();
                            candidate[key] = value.clone();
                            if serde_json::from_value::<Self>(candidate.clone())
                                .is_ok_and(|s| s.validate().is_ok())
                            {
                                values = candidate;
                                has_long_shortcut |= key == "long_recording_hotkey";
                                has_bookmark_shortcut |= key == "bookmark_hotkey";
                            }
                        }
                    }
                }
                let mut settings: Self = serde_json::from_value(values).unwrap();
                settings.restore_official_games();
                settings.recording_mode = settings.recording_mode.preference();
                settings.restore_shortcut_defaults(!has_long_shortcut, !has_bookmark_shortcut);
                settings.ensure_capture_action_rows();
                settings.sync_primary_bindings();
                if let Some(folder) = folder {
                    settings.clips_dir = folder;
                }
                (
                    settings,
                    format!(
                        "Some settings could not be loaded: {error:#}. {}",
                        if recovered {
                            "Your compatible preferences and saved storage folder have been kept."
                        } else {
                            "Choose a storage folder in Settings."
                        }
                    ),
                )
            }
        }
    }
}

fn read_preferences(path: &Path) -> serde_json::Value {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}))
}
fn merge_preferences(target: &mut serde_json::Value, source: serde_json::Value) {
    if let (Some(target), Some(source)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source {
            if value.is_object() && target.get(key).is_some_and(serde_json::Value::is_object) {
                merge_preferences(target.get_mut(key).unwrap(), value.clone());
            } else {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path).context("Could not save settings")
}

fn stored_folder(settings_path: &Path) -> Option<PathBuf> {
    let storage_path = settings_path.with_file_name("storage.json");
    // An older app may have updated settings.json after storage.json was written.
    let modified = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let paths = if modified(settings_path) >= modified(&storage_path) {
        [settings_path, storage_path.as_path()]
    } else {
        [storage_path.as_path(), settings_path]
    };
    paths.into_iter().find_map(|path| {
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let folder = PathBuf::from(value.get("clips_dir")?.as_str()?);
        (!folder.as_os_str().is_empty()).then_some(folder)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_is_first_run_only_and_completion_or_skip_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(!Settings::load_from(&path).unwrap().onboarding_completed);
        std::fs::write(&path, r#"{"height":720}"#).unwrap();
        assert!(Settings::load_from(&path).unwrap().onboarding_completed);
        for completed in [false, true] {
            let settings = Settings {
                onboarding_completed: completed,
                ..Settings::default()
            };
            settings.save_to(&path).unwrap();
            assert_eq!(
                Settings::load_from(&path).unwrap().onboarding_completed,
                completed
            );
            assert_eq!(
                Settings::recover_from(&path).0.onboarding_completed,
                completed
            );
        }
    }
    #[test]
    fn game_profiles_validate_before_mutating_preferences() {
        let mut settings = Settings::default();
        let before = settings.profiles.clone();
        for (name, exe) in [
            ("Game", "C:\\Game.exe"),
            ("Game", "Game/test.exe"),
            ("Game", "Game.txt"),
            (" ", "Game.exe"),
            ("Duplicate", "CS2.EXE"),
        ] {
            assert!(settings.add_game_profile(name, exe).is_err());
            assert_eq!(settings.profiles, before);
        }
        settings
            .add_game_profile(" My game ", " MyGame.exe ")
            .unwrap();
        assert_eq!(settings.profiles.last().unwrap().name, "My game");
        assert_eq!(settings.profiles.last().unwrap().executable, "MyGame.exe");
    }
    #[test]
    fn capture_state_and_library_preferences_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for mode in [
            RecordingMode::Clips,
            RecordingMode::AutomaticSessions,
            RecordingMode::Off,
        ] {
            for enabled in [false, true] {
                let settings = Settings {
                    recording_mode: mode,
                    capture_enabled: Some(enabled),
                    library_compact: true,
                    library_group_by_game: true,
                    library_sort: LibrarySort::Largest,
                    ..Settings::default()
                };
                settings.save_to(&path).unwrap();
                let restored = Settings::load_from(&path).unwrap();
                assert_eq!(restored.capture_enabled, Some(enabled));
                for background in [false, true] {
                    assert_eq!(
                        restored.start_on_launch(background),
                        enabled && mode != RecordingMode::Off
                    );
                }
                assert!(restored.library_compact);
                assert!(restored.library_group_by_game);
                assert_eq!(restored.library_sort, LibrarySort::Largest);
                assert!(!settings.recording_changed(&Settings {
                    capture_enabled: Some(!enabled),
                    library_sort: LibrarySort::Oldest,
                    library_compact: false,
                    library_group_by_game: false,
                    ..settings.clone()
                }));
            }
        }
    }

    #[test]
    fn legacy_preferences_keep_their_startup_behavior() {
        for mode in [
            RecordingMode::Clips,
            RecordingMode::AutomaticSessions,
            RecordingMode::Off,
        ] {
            let settings: Settings =
                serde_json::from_value(serde_json::json!({"recording_mode": mode})).unwrap();
            assert_eq!(settings.capture_enabled, None);
            for background in [false, true] {
                assert_eq!(
                    settings.start_on_launch(background),
                    mode.start_on_launch(background)
                );
            }
        }
    }
    #[test]
    fn added_unset_and_long_press_hotkeys_survive_restart() {
        use crate::capture_hotkeys::{CaptureAction, CaptureHotkey, HotkeyTrigger};
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("settings.json");
        let mut settings = Settings::default();
        settings.edit_capture_bindings()[0].key.clear();
        settings.edit_capture_bindings()[1].trigger = HotkeyTrigger::LongPress;
        settings.edit_capture_bindings().push(CaptureHotkey {
            id: 5,
            action: CaptureAction::Screenshot,
            key: "Control+F10".into(),
            ..CaptureHotkey::default()
        });
        settings.sync_primary_bindings();
        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path).unwrap();
        assert_eq!(loaded.capture_bindings(), settings.capture_bindings());
        settings.recording_hotkeys = Some(Vec::new());
        settings.sync_primary_bindings();
        settings.save_to(&path).unwrap();
        let restored = Settings::load_from(&path).unwrap().capture_bindings();
        assert_eq!(restored.len(), 4);
        assert!(restored.iter().all(|binding| binding.key.is_empty()));
    }
    #[test]
    fn legacy_full_mode_becomes_manual_and_existing_clip_shortcut_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"recording_mode":"FullRecording","hotkey":"F4"}"#).unwrap();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.recording_mode, RecordingMode::Clips);
        assert_eq!(settings.hotkey, "F4");
        assert_eq!(settings.long_recording_hotkey, "Alt+F7");
        assert_eq!(settings.bookmark_hotkey, "F6");
        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path).unwrap();
        assert_eq!(loaded.long_recording_hotkey, settings.long_recording_hotkey);
        assert_eq!(loaded.bookmark_hotkey, settings.bookmark_hotkey);
        let mut changed = loaded.clone();
        changed.long_recording_hotkey = "Alt+F10".into();
        changed.bookmark_hotkey = "F11".into();
        assert!(!changed.recording_changed(&loaded));
    }
    #[test]
    fn official_games_are_restored_without_losing_custom_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"profiles":[{"name":"My game","executable":"custom.exe"},{"name":"Old CS2","executable":"CS2.EXE"}]}"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path).unwrap();
        let count = OFFICIAL_GAMES.len();
        assert_eq!(loaded.profiles.len(), count + 1);
        assert!(
            loaded.profiles[..count]
                .iter()
                .all(GameProfile::is_official)
        );
        assert_eq!(loaded.profiles[3].name, "Counter-Strike 2");
        assert_eq!(loaded.profiles[count].executable, "custom.exe");
        assert!(!loaded.profiles[count].is_official());
        loaded.save_to(&path).unwrap();
        assert_eq!(
            Settings::load_from(&path).unwrap().profiles,
            loaded.profiles
        );

        std::fs::write(&path, r#"{"version":99,"profiles":[]}"#).unwrap();
        let (recovered, _) = Settings::recover_from(&path);
        assert_eq!(recovered.profiles.len(), count);
        assert!(recovered.profiles.iter().all(GameProfile::is_official));
    }

    #[test]
    fn sensitivity_migrates_and_suppression_modes_persist_independently() {
        let legacy: Settings =
            serde_json::from_str(r#"{"noise_suppression":true,"noise_threshold_db":-57.0}"#)
                .unwrap();
        assert!(legacy.input_sensitivity_enabled);
        assert_eq!(legacy.input_sensitivity_db, -57.0);
        assert_eq!(legacy.noise_suppression_mode, NoiseSuppression::None);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for mode in [
            NoiseSuppression::None,
            NoiseSuppression::Standard,
            NoiseSuppression::Neural,
        ] {
            let mut changed = legacy.clone();
            changed.noise_suppression_mode = mode;
            changed.save_to(&path).unwrap();
            let loaded = Settings::load_from(&path).unwrap();
            assert_eq!(loaded.noise_suppression_mode, mode);
            assert_eq!(loaded.input_sensitivity_db, -57.0);
            assert!(loaded.input_sensitivity_enabled);
            assert_eq!(
                changed.recording_changed(&legacy),
                mode != NoiseSuppression::None
            );
        }
    }
    #[test]
    fn preferences_survive_older_versions_and_recover_individual_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            mic_volume: 0.63,
            desktop_volume: 0.42,
            input_sensitivity_db: -65.0,
            input_sensitivity_enabled: true,
            push_to_talk: true,
            separate_audio_tracks: true,
            storage_limit_gb: Some(100),
            hotkey: "F10".into(),
            ..Settings::default()
        };
        settings.save_to(&path).unwrap();
        // An older executable writes only fields it understands.
        std::fs::write(
            &path,
            r#"{"version":1,"desktop_volume":0.8,"future_feature":{"enabled":true}}"#,
        )
        .unwrap();
        let loaded = Settings::recover_from(&path).0;
        assert_eq!(loaded.mic_volume, 0.63);
        assert_eq!(loaded.desktop_volume, 0.8);
        assert_eq!(loaded.storage_limit_gb, Some(100));
        assert!(loaded.separate_audio_tracks);
        loaded.save_to(&path).unwrap();
        assert_eq!(read_preferences(&path)["future_feature"]["enabled"], true);
        std::fs::write(
            &path,
            r#"{"version":99,"fps":999,"hotkey":"F11","microphone":true}"#,
        )
        .unwrap();
        let (recovered, warning) = Settings::recover_from(&path);
        assert!(!warning.is_empty());
        assert_eq!(recovered.fps, 60);
        assert_eq!(recovered.hotkey, "F11");
        assert_eq!(recovered.input_sensitivity_db, -65.0);
        assert_eq!(recovered.mic_volume, 0.63);
        std::fs::remove_file(&path).unwrap();
        let restored = Settings::recover_from(&path).0;
        assert_eq!(restored.mic_volume, 0.63);
        assert_eq!(restored.hotkey, "F10");
    }
    #[test]
    fn recording_audio_selection_persists() {
        let old: Settings = serde_json::from_str(r#"{"version":1,"desktop_audio":false,"output_device":"missing-headset","microphone":true}"#).unwrap();
        assert!(!old.desktop_audio);
        assert!(old.microphone);
        let json = serde_json::to_value(&old).unwrap();
        assert_eq!(json.get("desktop_audio").unwrap(), false);
        assert!(json.get("output_device").is_none());
        assert_eq!(old.clip_feedback, ClipFeedback::default());
    }
    #[test]
    fn feedback_preferences_persist_without_resetting_the_recording() {
        let original = Settings::default();
        let mut changed = original.clone();
        changed.clip_feedback = ClipFeedback {
            sound: false,
            notification: true,
            volume: 0.25,
        };
        assert!(!changed.recording_changed(&original));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        changed.save_to(&path).unwrap();
        assert_eq!(
            Settings::load_from(&path).unwrap().clip_feedback,
            changed.clip_feedback
        );
        changed.clip_feedback.volume = f32::NAN;
        assert!(changed.validate().is_err());
        changed.clip_feedback.volume = 1.01;
        assert!(changed.validate().is_err());
    }
    #[test]
    fn mono_preserves_single_input_level_and_old_settings() {
        assert_eq!(MicChannel::Left.mono([0.8, 0.0]), [0.8, 0.8]);
        assert_eq!(MicChannel::Right.mono([0.0, 0.7]), [0.7, 0.7]);
        assert_eq!(MicChannel::Mix.mono([1.0, -0.5]), [0.25, 0.25]);
        let old: Settings = serde_json::from_str("{\"version\":1}").unwrap();
        assert!(!old.mic_mono);
        assert_eq!(old.mic_channel, MicChannel::Left);
        let mut changed = old.clone();
        changed.mic_mono = true;
        assert!(changed.recording_changed(&old));
    }
    #[test]
    fn defaults_roundtrip() {
        let s: Settings =
            serde_json::from_str(&serde_json::to_string(&Settings::default()).unwrap()).unwrap();
        assert_eq!(s.fps, 60);
        assert_eq!(s.replay_seconds, 60);
        assert_eq!(s.recording_mode, RecordingMode::Clips);
        assert!(!s.close_to_tray);
        s.validate().unwrap();
    }
    #[test]
    fn close_to_tray_is_opt_in_and_persists_without_restarting_capture() {
        let old: Settings = serde_json::from_str(r#"{"version":1}"#).unwrap();
        assert!(!old.close_to_tray);
        let mut changed = old.clone();
        changed.close_to_tray = true;
        assert!(!changed.recording_changed(&old));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        changed.save_to(&path).unwrap();
        assert!(Settings::recover_from(&path).0.close_to_tray);
    }
    #[test]
    fn full_recording_is_optional_and_changes_capture() {
        let old: Settings = serde_json::from_str("{\"version\":1}").unwrap();
        assert_eq!(old.recording_mode, RecordingMode::Clips);
        let mut full = old.clone();
        full.recording_mode = RecordingMode::FullRecording;
        assert!(full.recording_changed(&old));
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&full).unwrap()).unwrap();
        assert_eq!(restored.recording_mode, RecordingMode::FullRecording);
    }
    #[test]
    fn storage_survives_repeated_saves_missing_settings_and_incompatible_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            clips_dir: PathBuf::from("G:\\gavnel"),
            ..Settings::default()
        };
        settings.save_to(&path).unwrap();
        settings.save_to(&path).unwrap();
        assert_eq!(
            Settings::load_from(&path).unwrap().clips_dir,
            settings.clips_dir
        );
        std::fs::write(
            &path,
            r#"{"version":99,"clips_dir":"G:\\gavnel","recording_mode":"FutureMode"}"#,
        )
        .unwrap();
        let (recovered, warning) = Settings::recover_from(&path);
        assert_eq!(recovered.clips_dir, settings.clips_dir);
        assert!(!warning.is_empty());
        std::fs::write(&path, b"interrupted write").unwrap();
        assert_eq!(
            Settings::recover_from(&path).0.clips_dir,
            settings.clips_dir
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            Settings::recover_from(&path).0.clips_dir,
            settings.clips_dir
        );
        assert!(Settings::path().is_absolute());
    }
    #[test]
    fn automatic_sessions_and_recording_off_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for mode in [RecordingMode::AutomaticSessions, RecordingMode::Off] {
            let settings = Settings {
                recording_mode: mode,
                clips_dir: PathBuf::from("G:\\gavnel"),
                ..Settings::default()
            };
            settings.save_to(&path).unwrap();
            let restored = Settings::recover_from(&path).0;
            assert_eq!(restored.recording_mode, mode);
            assert_eq!(restored.clips_dir, settings.clips_dir);
            assert_eq!(
                restored.recording_mode.start_on_launch(true),
                mode == RecordingMode::AutomaticSessions
            );
            assert_eq!(
                restored.recording_mode.start_on_launch(false),
                mode == RecordingMode::AutomaticSessions
            );
        }
    }
    #[test]
    fn invalid_settings_rejected() {
        let mut s = Settings {
            fps: 500,
            ..Settings::default()
        };
        assert!(s.validate().is_err());
        s.fps = 60;
        s.version = 9;
        assert!(s.validate().is_err());
        s.version = 1;
        s.mic_volume = f32::NAN;
        assert!(s.validate().is_err());
    }
    #[test]
    fn folder_hotkey_and_startup_changes_preserve_the_recording() {
        let original = Settings::default();
        let mut changed = original.clone();
        changed.clips_dir = PathBuf::from("F:\\Clips");
        changed.hotkey = "F9".into();
        changed.start_with_windows = true;
        changed.close_to_tray = true;
        assert!(!changed.recording_changed(&original));
        changed.height = 720;
        assert!(changed.recording_changed(&original));
    }
}
