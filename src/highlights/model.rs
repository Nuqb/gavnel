use crate::replay::TICKS;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Game {
    Cs2,
    Fortnite,
    Valorant,
    Rust,
}
impl Game {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cs2 => "Counter-Strike 2",
            Self::Fortnite => "Fortnite",
            Self::Valorant => "Valorant",
            Self::Rust => "Rust",
        }
    }
    pub fn from_executable(exe: &str) -> Option<Self> {
        let e = exe.to_ascii_lowercase();
        if e == "cs2.exe" {
            Some(Self::Cs2)
        } else if e.starts_with("fortniteclient-") {
            Some(Self::Fortnite)
        } else if e == "valorant-win64-shipping.exe" || e == "valorant.exe" {
            Some(Self::Valorant)
        } else if e == "rustclient.exe" {
            Some(Self::Rust)
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventKind {
    Kill,
    Death,
    Assist,
    RoundStart,
    RoundWin,
    RoundLoss,
    MatchStart,
    MatchEnd,
    Victory,
    Wounded,
}
impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Kill => "Kill",
            Self::Death => "Death",
            Self::Assist => "Assist",
            Self::RoundStart => "Round start",
            Self::RoundWin => "Round won",
            Self::RoundLoss => "Round lost",
            Self::MatchStart => "Match start",
            Self::MatchEnd => "Match end",
            Self::Victory => "Victory",
            Self::Wounded => "Wounded",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameEvent {
    pub game: Game,
    pub kind: EventKind,
    pub ticks: u64,
    pub source: String,
    pub confidence: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VisualRule {
    pub kind: EventKind,
    pub region: [f32; 4],
    pub phrases: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameSettings {
    pub game: Game,
    pub enabled: bool,
    pub events: Vec<EventKind>,
    pub rules: Vec<VisualRule>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightSettings {
    pub before: u32,
    pub after: u32,
    pub games: Vec<GameSettings>,
    pub cs2_token: String,
    pub cs2_port: u16,
}
impl Default for HighlightSettings {
    fn default() -> Self {
        Self {
            before: 15,
            after: 5,
            cs2_token: String::new(),
            cs2_port: 19877,
            games: [Game::Cs2, Game::Fortnite, Game::Valorant, Game::Rust]
                .into_iter()
                .map(|game| GameSettings {
                    game,
                    enabled: false,
                    events: match game {
                        Game::Cs2 => vec![
                            EventKind::Kill,
                            EventKind::Death,
                            EventKind::Assist,
                            EventKind::RoundStart,
                            EventKind::RoundWin,
                            EventKind::RoundLoss,
                            EventKind::MatchStart,
                            EventKind::MatchEnd,
                        ],
                        Game::Fortnite => {
                            vec![EventKind::Kill, EventKind::Death, EventKind::Victory]
                        }
                        Game::Valorant => vec![
                            EventKind::Kill,
                            EventKind::Death,
                            EventKind::RoundWin,
                            EventKind::RoundLoss,
                        ],
                        Game::Rust => vec![EventKind::Death, EventKind::Wounded],
                    },
                    rules: default_rules(game),
                })
                .collect(),
        }
    }
}
fn default_rules(game: Game) -> Vec<VisualRule> {
    let rule = |kind, region, words: &[&str]| VisualRule {
        kind,
        region,
        phrases: words.iter().map(|w| (*w).into()).collect(),
    };
    match game {
        Game::Cs2 => vec![],
        Game::Fortnite => vec![
            rule(EventKind::Kill, [0.32, 0.62, 0.36, 0.20], &["eliminated"]),
            rule(
                EventKind::Death,
                [0.2, 0.0, 0.6, 0.22],
                &["you placed", "eliminated by"],
            ),
            rule(
                EventKind::Victory,
                [0.20, 0.0, 0.60, 0.25],
                &["victory royale"],
            ),
        ],
        Game::Valorant => vec![
            rule(
                EventKind::Kill,
                [0.40, 0.65, 0.20, 0.22],
                &["headshot", "kill"],
            ),
            rule(EventKind::Death, [0.65, 0.10, 0.35, 0.40], &["killed by"]),
            rule(
                EventKind::RoundWin,
                [0.30, 0.05, 0.40, 0.30],
                &["won", "victory"],
            ),
            rule(
                EventKind::RoundLoss,
                [0.30, 0.05, 0.40, 0.30],
                &["lost", "defeat"],
            ),
        ],
        Game::Rust => vec![
            rule(EventKind::Death, [0.0, 0.0, 0.6, 0.25], &["dead"]),
            rule(EventKind::Wounded, [0.25, 0.15, 0.5, 0.4], &["wounded"]),
        ],
    }
}
impl HighlightSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.before <= 120
                && self.after <= 60
                && self.before + self.after > 0
                && self.cs2_port >= 1024,
            "Invalid automatic highlight settings."
        );
        for game in &self.games {
            anyhow::ensure!(game.rules.len() <= 12, "Too many visual highlight rules.");
            for r in &game.rules {
                let [x, y, w, h] = r.region;
                anyhow::ensure!(
                    r.region.iter().all(|v| v.is_finite())
                        && x >= 0.0
                        && y >= 0.0
                        && w > 0.01
                        && h > 0.01
                        && x + w <= 1.001
                        && y + h <= 1.001
                        && !r.phrases.is_empty()
                        && r.phrases
                            .iter()
                            .all(|s| !s.trim().is_empty() && s.len() <= 100),
                    "Invalid highlight calibration."
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct PendingWindow {
    pub start: u64,
    pub end: u64,
    pub events: Vec<GameEvent>,
}
pub fn retention_seconds(before: u32, after: u32) -> u32 {
    (before + after).max(60) + 2
}
pub fn merge_window(windows: &mut Vec<PendingWindow>, event: GameEvent, before: u32, after: u32) {
    let mut merged = PendingWindow {
        start: event.ticks.saturating_sub(before as u64 * TICKS),
        end: event.ticks + after as u64 * TICKS,
        events: vec![event],
    };
    let maximum = (before + after).max(60) as u64 * TICKS;
    let mut index = 0;
    while index < windows.len() {
        let existing = &windows[index];
        let start = merged.start.min(existing.start);
        let end = merged.end.max(existing.end);
        if merged.start <= existing.end && merged.end >= existing.start && end - start <= maximum {
            let existing = windows.remove(index);
            merged.start = start;
            merged.end = end;
            merged.events.extend(existing.events);
            merged.events.truncate(128);
            index = 0;
        } else {
            index += 1;
        }
    }
    if windows.len() < 8 {
        windows.push(merged);
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EventFile {
    pub version: u32,
    pub events: Vec<GameEvent>,
}
pub fn save_events(video: &std::path::Path, events: &[GameEvent]) -> anyhow::Result<()> {
    if events.is_empty() {
        return Ok(());
    }
    let p = video.with_extension("events.json");
    let old = load_events(video);
    let mut all = old;
    all.extend_from_slice(events);
    std::fs::write(
        p,
        serde_json::to_vec_pretty(&EventFile {
            version: 1,
            events: all,
        })?,
    )?;
    Ok(())
}
pub fn load_events(video: &std::path::Path) -> Vec<GameEvent> {
    std::fs::read(video.with_extension("events.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<EventFile>(&b).ok())
        .filter(|f| f.version == 1)
        .map(|f| f.events)
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_merge_and_keep_all_events() {
        let mut windows = vec![];
        for tick in [20, 23] {
            merge_window(
                &mut windows,
                GameEvent {
                    game: Game::Cs2,
                    kind: EventKind::Kill,
                    ticks: tick * TICKS,
                    source: "GSI".into(),
                    confidence: 1.0,
                },
                15,
                5,
            );
        }
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].start, 5 * TICKS);
        assert_eq!(windows[0].end, 28 * TICKS);
        assert_eq!(windows[0].events.len(), 2);
    }
    #[test]
    fn defaults_are_opt_in_and_game_specific() {
        let s = HighlightSettings::default();
        s.validate().unwrap();
        assert!(s.games.iter().all(|g| !g.enabled));
        assert!(
            !s.games
                .iter()
                .find(|g| g.game == Game::Rust)
                .unwrap()
                .events
                .contains(&EventKind::Kill)
        );
    }
}
