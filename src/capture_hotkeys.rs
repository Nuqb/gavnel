use crate::{config::Settings, engine::Command};
use global_hotkey::hotkey::HotKey;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
pub const LONG_PRESS_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureAction {
    Clip,
    LongRecording,
    Bookmark,
    Screenshot,
}
impl CaptureAction {
    pub const ALL: [Self; 4] = [
        Self::Clip,
        Self::LongRecording,
        Self::Bookmark,
        Self::Screenshot,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Clip => "Clip",
            Self::LongRecording => "Start/stop long recording",
            Self::Bookmark => "Bookmark",
            Self::Screenshot => "Screenshot",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyTrigger {
    #[default]
    Tap,
    LongPress,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureHotkey {
    pub id: u64,
    pub action: CaptureAction,
    pub key: String,
    pub trigger: HotkeyTrigger,
    pub clip_seconds: u32,
}
impl Default for CaptureHotkey {
    fn default() -> Self {
        Self {
            id: 1,
            action: CaptureAction::Clip,
            key: String::new(),
            trigger: HotkeyTrigger::Tap,
            clip_seconds: 60,
        }
    }
}
impl CaptureHotkey {
    pub fn command(&self, settings: &Settings) -> Command {
        match self.action {
            CaptureAction::Clip => Command::SaveFor(self.clip_seconds),
            CaptureAction::LongRecording => {
                Command::ToggleLongRecording(Box::new(settings.clone()))
            }
            CaptureAction::Bookmark => Command::Bookmark,
            CaptureAction::Screenshot => Command::Screenshot(Box::new(settings.clone())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredShortcut {
    pub key: HotKey,
    pub binding: CaptureHotkey,
}

/// Tokens prevent a delayed hold from firing after release, rebind, or unset.
#[derive(Default)]
pub struct PressedShortcuts {
    generation: u64,
    held: HashMap<u32, (u64, Instant, bool)>,
}
impl PressedShortcuts {
    pub fn press(&mut self, key: u32, now: Instant) -> Option<u64> {
        if self.held.contains_key(&key) {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.held.insert(key, (self.generation, now, false));
        Some(self.generation)
    }
    pub fn release(&mut self, key: u32, now: Instant) -> Option<(Duration, bool)> {
        self.held
            .remove(&key)
            .map(|(_, start, fired)| (now.saturating_duration_since(start), fired))
    }
    pub fn fire_long(&mut self, key: u32, token: u64, now: Instant) -> bool {
        if let Some((current, start, fired)) = self.held.get_mut(&key)
            && *current == token
            && !*fired
            && now.saturating_duration_since(*start) >= LONG_PRESS_DELAY
        {
            *fired = true;
            return true;
        }
        false
    }
    pub fn clear(&mut self) {
        self.held.clear();
    }
}

impl Settings {
    /// Keep one row for each action without reassigning a key the user cleared.
    pub fn ensure_capture_action_rows(&mut self) {
        let clip_seconds = self.replay_seconds;
        let Some(bindings) = &mut self.recording_hotkeys else {
            return;
        };
        for action in CaptureAction::ALL {
            if !bindings.iter().any(|binding| binding.action == action) {
                let id = (1..)
                    .find(|id| !bindings.iter().any(|binding| binding.id == *id))
                    .unwrap();
                bindings.push(CaptureHotkey {
                    id,
                    action,
                    clip_seconds,
                    ..CaptureHotkey::default()
                });
            }
        }
    }

    pub fn remove_capture_binding(&mut self, id: u64) -> bool {
        let bindings = self.capture_bindings();
        let Some(binding) = bindings.iter().find(|binding| binding.id == id) else {
            return false;
        };
        if bindings
            .iter()
            .filter(|other| other.action == binding.action)
            .count()
            <= 1
        {
            return false;
        }
        self.edit_capture_bindings()
            .retain(|binding| binding.id != id);
        self.sync_primary_bindings();
        true
    }
    pub fn capture_bindings(&self) -> Vec<CaptureHotkey> {
        self.recording_hotkeys.clone().unwrap_or_else(|| {
            let used = [
                &self.hotkey,
                &self.long_recording_hotkey,
                &self.bookmark_hotkey,
            ];
            let screenshot = ["F7", "Control+F7", "F10", "Alt+F8"]
                .into_iter()
                .find(|candidate| {
                    !used
                        .iter()
                        .any(|key| key.parse::<HotKey>().ok() == candidate.parse::<HotKey>().ok())
                })
                .unwrap_or("Control+F7");
            CaptureAction::ALL
                .into_iter()
                .enumerate()
                .map(|(i, action)| CaptureHotkey {
                    id: i as u64 + 1,
                    action,
                    key: match action {
                        CaptureAction::Clip => self.hotkey.clone(),
                        CaptureAction::LongRecording => self.long_recording_hotkey.clone(),
                        CaptureAction::Bookmark => self.bookmark_hotkey.clone(),
                        CaptureAction::Screenshot => screenshot.into(),
                    },
                    trigger: HotkeyTrigger::Tap,
                    clip_seconds: self.replay_seconds,
                })
                .collect()
        })
    }
    pub fn edit_capture_bindings(&mut self) -> &mut Vec<CaptureHotkey> {
        if self.recording_hotkeys.is_none() {
            self.recording_hotkeys = Some(self.capture_bindings());
        }
        self.recording_hotkeys.as_mut().unwrap()
    }
    pub fn sync_primary_bindings(&mut self) {
        let Some(bindings) = &self.recording_hotkeys else {
            return;
        };
        let primary = |action| {
            bindings
                .iter()
                .find(|b| b.action == action && !b.key.is_empty())
                .map(|b| b.key.clone())
                .unwrap_or_default()
        };
        self.hotkey = primary(CaptureAction::Clip);
        self.long_recording_hotkey = primary(CaptureAction::LongRecording);
        self.bookmark_hotkey = primary(CaptureAction::Bookmark);
        if let Some(clip) = bindings
            .iter()
            .find(|b| b.action == CaptureAction::Clip && !b.key.is_empty())
            .or_else(|| bindings.iter().find(|b| b.action == CaptureAction::Clip))
        {
            self.replay_seconds = clip.clip_seconds;
        }
    }
    pub fn replay_buffer_seconds(&self) -> u32 {
        self.capture_bindings()
            .iter()
            .filter(|b| b.action == CaptureAction::Clip && !b.key.is_empty())
            .map(|b| b.clip_seconds)
            .max()
            .unwrap_or(self.replay_seconds)
            .max(self.replay_seconds)
    }
    pub fn primary_clip_id(&mut self) -> u64 {
        if let Some(clip) = self
            .edit_capture_bindings()
            .iter()
            .find(|b| b.action == CaptureAction::Clip)
        {
            return clip.id;
        }
        let id = self
            .edit_capture_bindings()
            .iter()
            .map(|b| b.id)
            .max()
            .unwrap_or(0)
            + 1;
        let clip_seconds = self.replay_seconds;
        self.edit_capture_bindings().push(CaptureHotkey {
            id,
            clip_seconds,
            ..CaptureHotkey::default()
        });
        id
    }
    pub fn set_primary_clip_seconds(&mut self, seconds: u32) {
        self.replay_seconds = seconds;
        let bindings = self.edit_capture_bindings();
        let index = bindings
            .iter()
            .position(|b| b.action == CaptureAction::Clip && !b.key.is_empty())
            .or_else(|| {
                bindings
                    .iter()
                    .position(|b| b.action == CaptureAction::Clip)
            });
        if let Some(index) = index {
            bindings[index].clip_seconds = seconds;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_extra_rows_of_the_same_action_can_be_removed() {
        for action in CaptureAction::ALL {
            let mut settings = Settings::default();
            let binding = settings
                .capture_bindings()
                .into_iter()
                .find(|binding| binding.action == action)
                .unwrap();
            assert!(!settings.remove_capture_binding(binding.id));
            settings.edit_capture_bindings().push(CaptureHotkey {
                id: 5,
                ..binding.clone()
            });
            assert!(settings.remove_capture_binding(binding.id));
            assert!(!settings.remove_capture_binding(5));
            assert_eq!(
                settings
                    .capture_bindings()
                    .iter()
                    .filter(|binding| binding.action == action)
                    .count(),
                1
            );
        }
    }
    #[test]
    fn releases_and_rebinds_cancel_pending_holds_and_repeats_do_not_retrigger() {
        let mut held = PressedShortcuts::default();
        let now = Instant::now();
        let first = held.press(7, now).unwrap();
        assert!(held.press(7, now).is_none());
        held.release(7, now);
        assert!(!held.fire_long(7, first, now + LONG_PRESS_DELAY));
        let second = held.press(7, now).unwrap();
        assert_ne!(first, second);
        held.clear();
        assert!(!held.fire_long(7, second, now + LONG_PRESS_DELAY));
    }
    #[test]
    fn long_press_fires_once_at_threshold_and_short_release_stays_a_tap() {
        let now = Instant::now();
        let mut held = PressedShortcuts::default();
        let token = held.press(7, now).unwrap();
        assert!(!held.fire_long(7, token, now + Duration::from_millis(499)));
        assert!(held.fire_long(7, token, now + LONG_PRESS_DELAY));
        assert!(!held.fire_long(7, token, now + Duration::from_secs(1)));
        assert_eq!(
            held.release(7, now + Duration::from_secs(1)),
            Some((Duration::from_secs(1), true))
        );
        held.press(7, now);
        assert_eq!(
            held.release(7, now + Duration::from_millis(100)),
            Some((Duration::from_millis(100), false))
        );
    }
}
