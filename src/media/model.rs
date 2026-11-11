use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTrack {
    pub index: u32,
    pub label: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub audio: Vec<AudioTrack>,
    #[serde(default)]
    pub markers: Vec<(f64, String)>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Aspect {
    #[default]
    Original,
    Wide,
    Vertical,
    Square,
    Custom,
}
impl Aspect {
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::Wide => "16:9",
            Self::Vertical => "9:16",
            Self::Square => "Square",
            Self::Custom => "Custom",
        }
    }
    pub fn ratio(self, info: &MediaInfo) -> f64 {
        match self {
            Self::Original | Self::Custom => info.width as f64 / info.height as f64,
            Self::Wide => 16.0 / 9.0,
            Self::Vertical => 9.0 / 16.0,
            Self::Square => 1.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub aspect: Aspect,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub custom_rect: Option<[f64; 4]>,
}
impl Default for Crop {
    fn default() -> Self {
        Self {
            aspect: Aspect::Original,
            x: 0.5,
            y: 0.5,
            custom_rect: None,
        }
    }
}
impl Crop {
    pub fn normalized(&self, info: &MediaInfo) -> [f64; 4] {
        let (x, y, w, h) = self.pixels(info);
        [
            x as f64 / info.width as f64,
            y as f64 / info.height as f64,
            w as f64 / info.width as f64,
            h as f64 / info.height as f64,
        ]
    }
    pub fn set_rect(&mut self, rect: [f64; 4]) {
        self.aspect = Aspect::Custom;
        self.custom_rect = Some(rect);
    }
    pub fn center(&mut self) {
        self.x = 0.5;
        self.y = 0.5;
        if let Some([_, _, w, h]) = self.custom_rect {
            self.custom_rect = Some([(1.0 - w) * 0.5, (1.0 - h) * 0.5, w, h]);
        }
    }
    pub fn pixels(&self, info: &MediaInfo) -> (u32, u32, u32, u32) {
        if self.aspect == Aspect::Custom
            && let Some([left, top, width, height]) = self.custom_rect
        {
            let w = ((width * info.width as f64).round() as u32 & !1).clamp(2, info.width & !1);
            let h = ((height * info.height as f64).round() as u32 & !1).clamp(2, info.height & !1);
            let x = ((left * info.width as f64).round() as u32 & !1).min((info.width - w) & !1);
            let y = ((top * info.height as f64).round() as u32 & !1).min((info.height - h) & !1);
            return (x, y, w, h);
        }
        let ratio = self.aspect.ratio(info);
        let w = (info.width as f64).min(info.height as f64 * ratio).floor() as u32 & !1;
        let h = (info.height as f64).min(info.width as f64 / ratio).floor() as u32 & !1;
        (
            ((info.width - w) as f64 * self.x.clamp(0.0, 1.0)) as u32 & !1,
            ((info.height - h) as f64 * self.y.clamp(0.0, 1.0)) as u32 & !1,
            w.max(2),
            h.max(2),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackMix {
    pub index: u32,
    pub label: String,
    pub gain: f64,
    pub muted: bool,
    pub solo: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipSegment {
    pub info: MediaInfo,
    pub start: f64,
    pub end: f64,
    pub crop: Crop,
    pub audio: Vec<TrackMix>,
}
impl ClipSegment {
    pub fn new(info: MediaInfo) -> Self {
        let audio = info
            .audio
            .iter()
            .map(|a| TrackMix {
                index: a.index,
                label: a.label.clone(),
                gain: 1.0,
                muted: a.label.starts_with("Audio ") && a.index != info.audio[0].index,
                solo: false,
            })
            .collect();
        Self {
            end: info.duration,
            start: 0.0,
            crop: Crop::default(),
            info,
            audio,
        }
    }
    pub fn bookmark(&mut self, time: f64) {
        self.start = (time - 15.0).max(0.0);
        self.end = (time + 5.0).min(self.info.duration);
    }
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
    pub fn gain(&self, track: &TrackMix) -> f64 {
        if track.muted || self.audio.iter().any(|a| a.solo) && !track.solo {
            0.0
        } else {
            track.gain
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditProject {
    pub version: u32,
    pub name: String,
    pub segments: Vec<ClipSegment>,
}
impl EditProject {
    pub fn new(segments: Vec<ClipSegment>) -> Self {
        Self {
            version: 1,
            name: "Untitled".into(),
            segments,
        }
    }
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(ClipSegment::duration).sum()
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "This project version is not supported.");
        ensure!(
            !self.segments.is_empty() && self.segments.len() <= 100,
            "Choose between 1 and 100 clips."
        );
        for s in &self.segments {
            ensure!(
                s.info.path.is_file(),
                "Source video is missing: {}",
                s.info.path.display()
            );
            ensure!(
                s.info.duration.is_finite()
                    && s.info.duration > 0.0
                    && s.info.width >= 2
                    && s.info.height >= 2
                    && s.info.fps.is_finite()
                    && s.info.fps > 0.0,
                "Invalid source video."
            );
            ensure!(
                s.start.is_finite()
                    && s.end.is_finite()
                    && s.start >= 0.0
                    && s.end <= s.info.duration + 0.001
                    && s.duration() >= 1.0 / s.info.fps,
                "Choose a selection at least one frame long."
            );
            ensure!(
                s.crop.x.is_finite() && s.crop.y.is_finite(),
                "Invalid crop position."
            );
            if let Some([x, y, w, h]) = s.crop.custom_rect {
                ensure!(
                    [x, y, w, h].iter().all(|v| v.is_finite())
                        && x >= 0.0
                        && y >= 0.0
                        && w > 0.0
                        && h > 0.0
                        && x + w <= 1.000001
                        && y + h <= 1.000001,
                    "Invalid custom crop rectangle."
                );
            }
            for t in &s.audio {
                ensure!(
                    t.gain.is_finite()
                        && (0.0..=2.0).contains(&t.gain)
                        && s.info.audio.iter().any(|a| a.index == t.index),
                    "Invalid audio track."
                );
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        ensure!(
            path.extension().is_some_and(|e| e == "gavnel-project"),
            "Choose a .gavnel-project filename."
        );
        let temporary = path.with_extension("gavnel-project.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        use std::os::windows::ffi::OsStrExt;
        let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            windows::Win32::Storage::FileSystem::MoveFileExW(
                windows::core::PCWSTR(from.as_ptr()),
                windows::core::PCWSTR(to.as_ptr()),
                windows::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                    | windows::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
            )?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn load(path: &Path) -> Result<Self> {
        let project: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        project.validate()?;
        Ok(project)
    }
}
#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub replace_source: bool,
    pub path: PathBuf,
    pub height: Option<u32>,
    pub size_mb: Option<f64>,
    pub canvas: Option<(u32, u32, u32)>,
}
#[derive(Clone, Debug, Default)]
pub struct JobProgress {
    pub fraction: f32,
    pub message: String,
    pub result: Option<Result<PathBuf, String>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> MediaInfo {
        MediaInfo {
            path: PathBuf::new(),
            duration: 100.0,
            width: 1920,
            height: 1080,
            fps: 60.0,
            audio: vec![],
            markers: Vec::new(),
        }
    }
    #[test]
    fn crop_and_bookmark_bounds() {
        let mut s = ClipSegment::new(info());
        s.bookmark(3.0);
        assert_eq!((s.start, s.end), (0.0, 8.0));
        s.crop.aspect = Aspect::Vertical;
        let (x, y, w, h) = s.crop.pixels(&s.info);
        assert!(x + w <= 1920 && y + h <= 1080);
        assert_eq!(w % 2, 0);
        assert_eq!(h, 1080);
    }
    #[test]
    fn solo_never_doubles_the_mix() {
        let mut s = ClipSegment::new(info());
        s.audio = vec![
            TrackMix {
                index: 1,
                label: "Game".into(),
                gain: 1.0,
                muted: false,
                solo: true,
            },
            TrackMix {
                index: 2,
                label: "Microphone".into(),
                gain: 1.0,
                muted: false,
                solo: false,
            },
        ];
        assert_eq!(s.gain(&s.audio[1]), 0.0);
        assert_eq!(s.gain(&s.audio[0]), 1.0);
    }
}

#[cfg(test)]
mod crop_compatibility_tests {
    use super::*;
    #[test]
    fn old_crops_load_and_custom_rectangles_roundtrip() {
        let old: Crop = serde_json::from_str(r#"{"aspect":"Wide","x":0.5,"y":0.5}"#).unwrap();
        assert!(old.custom_rect.is_none());
        let info = MediaInfo {
            path: PathBuf::new(),
            duration: 5.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            audio: vec![],
            markers: vec![],
        };
        let mut crop = old;
        crop.set_rect([0.1, 0.2, 0.5, 0.4]);
        assert_eq!(crop.aspect, Aspect::Custom);
        let (x, y, w, h) = crop.pixels(&info);
        assert_eq!((x, y, w, h), (192, 216, 960, 432));
        let saved = serde_json::to_string(&crop).unwrap();
        assert_eq!(serde_json::from_str::<Crop>(&saved).unwrap(), crop);
        crop.center();
        let rect = crop.normalized(&info);
        assert!((rect[0] - 0.25).abs() < 0.002 && (rect[1] - 0.3).abs() < 0.002);
    }
}
