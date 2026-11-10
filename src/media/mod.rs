mod model;
mod render;
#[cfg(test)]
mod tests;
mod worker;
use anyhow::{Context, Result, ensure};
pub use model::*;
use serde_json::Value;
use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
pub use worker::{MediaReply, MediaService};

#[derive(Clone)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
impl Tools {
    pub fn locate() -> Result<Self> {
        static VERIFIED: std::sync::OnceLock<std::result::Result<Tools, String>> =
            std::sync::OnceLock::new();
        VERIFIED
            .get_or_init(|| Self::find_verified().map_err(|e| e.to_string()))
            .clone()
            .map_err(anyhow::Error::msg)
    }
    fn find_verified() -> Result<Self> {
        let executable = std::env::current_exe()?;
        let roots = [
            executable
                .parent()
                .unwrap_or(Path::new("."))
                .join("media/bin"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("work/media/bin"),
        ];
        let bin = roots
            .into_iter()
            .find(|r| r.join("ffmpeg.exe").is_file() && r.join("ffprobe.exe").is_file())
            .context(
                "The editing tools are missing. Rebuild with the launcher or reinstall Gavnel.",
            )?;
        use sha2::{Digest, Sha256};
        let files: std::collections::BTreeMap<String, String> =
            serde_json::from_str(include_str!("../../packaging/media-files.json"))?;
        let runtime = bin.parent().context("Invalid media runtime path")?;
        for (name, expected) in files {
            let path = runtime.join(&name);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("Editing runtime file is missing: {name}"))?;
            ensure!(
                format!("{:x}", Sha256::digest(&bytes)) == expected,
                "Editing runtime verification failed for {name}. Rebuild or reinstall Gavnel."
            );
        }
        Ok(Self {
            ffmpeg: bin.join("ffmpeg.exe"),
            ffprobe: bin.join("ffprobe.exe"),
        })
    }
    pub fn command(&self, probe: bool) -> Command {
        let mut cmd = Command::new(if probe { &self.ffprobe } else { &self.ffmpeg });
        cmd.creation_flags(0x08004000).stdin(Stdio::null());
        cmd
    }
    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        let output = self
            .command(true)
            .args([
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
            ])
            .arg(path)
            .output()?;
        ensure!(
            output.status.success(),
            "Could not read video: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let data: Value = serde_json::from_slice(&output.stdout)?;
        let streams = data["streams"]
            .as_array()
            .context("No media streams found.")?;
        let video = streams
            .iter()
            .find(|s| s["codec_type"] == "video")
            .context("Choose a video file.")?;
        let duration = data["format"]["duration"]
            .as_str()
            .and_then(|v| v.parse::<f64>().ok())
            .context("Video duration is unavailable.")?;
        let rate = video["avg_frame_rate"].as_str().unwrap_or("30/1");
        let (n, d) = rate.split_once('/').unwrap_or((rate, "1"));
        let fps = n.parse::<f64>().unwrap_or(30.0) / d.parse::<f64>().unwrap_or(1.0);
        let audio_streams: Vec<_> = streams
            .iter()
            .filter(|s| s["codec_type"] == "audio")
            .collect();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let legacy_gavnel = audio_streams.len() == 3
            && stem.split('_').any(|part| {
                part.len() == 10
                    && part.as_bytes().get(4) == Some(&b'-')
                    && part.as_bytes().get(7) == Some(&b'-')
            });
        let mut audio: Vec<_> = audio_streams
            .iter()
            .enumerate()
            .map(|(i, s)| AudioTrack {
                index: s["index"].as_u64().unwrap_or(0) as u32,
                label: s["tags"]["title"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        if legacy_gavnel {
                            ["Combined", "PC Audio", "Microphone"][i].into()
                        } else {
                            format!("Audio {}", i + 1)
                        }
                    }),
            })
            .collect();
        if let Ok(bytes) = std::fs::read(path.with_extension("tracks.json"))
            && let Ok(labels) = serde_json::from_slice::<Vec<String>>(&bytes)
        {
            for (a, label) in audio.iter_mut().zip(labels) {
                a.label = label;
            }
        }
        if audio.len() > 1 && audio[0].label == "Combined" {
            audio.remove(0);
        }
        let events = crate::highlights::load_events(path);
        let mut markers: Vec<_> = crate::bookmarks::load(path)?
            .into_iter()
            .map(|time| {
                let label = events
                    .iter()
                    .find(|event| {
                        (event.ticks as f64 / crate::replay::TICKS as f64 - time).abs() < 0.01
                    })
                    .map(|event| event.kind.label().to_owned())
                    .unwrap_or_else(|| "Bookmark".into());
                (time, label)
            })
            .collect();
        for event in events {
            let seconds = event.ticks as f64 / crate::replay::TICKS as f64;
            if !markers
                .iter()
                .any(|(time, _)| (time - seconds).abs() < 0.01)
            {
                markers.push((seconds, event.kind.label().into()));
            }
        }
        markers.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(MediaInfo {
            markers,
            path: path.to_owned(),
            duration,
            width: video["width"].as_u64().unwrap_or(0) as u32,
            height: video["height"].as_u64().unwrap_or(0) as u32,
            fps: if fps.is_finite() && fps > 0.0 {
                fps
            } else {
                30.0
            },
            audio,
        })
    }
    pub fn filmstrip(&self, path: &Path) -> Result<Vec<eframe::egui::ColorImage>> {
        let info = self.probe(path)?;
        let width = 144;
        let height =
            ((width as f64 * info.height as f64 / info.width as f64).round() as usize).max(1);
        let mut frames = Vec::new();
        for i in 0..8 {
            let time = info.duration * (i as f64 + 0.5) / 8.0;
            let output = self
                .command(false)
                .args([
                    "-v",
                    "error",
                    "-ss",
                    &format!("{time:.6}"),
                    "-threads",
                    "1",
                    "-i",
                ])
                .arg(path)
                .args([
                    "-frames:v",
                    "1",
                    "-vf",
                    &format!("scale={width}:{height}"),
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgba",
                    "pipe:1",
                ])
                .output()?;
            ensure!(
                output.status.success() && output.stdout.len() == width * height * 4,
                "Could not load timeline thumbnails."
            );
            frames.push(eframe::egui::ColorImage::from_rgba_unmultiplied(
                [width, height],
                &output.stdout,
            ));
        }
        Ok(frames)
    }
    pub fn frame(&self, path: &Path, seconds: f64) -> Result<eframe::egui::ColorImage> {
        let info = self.probe(path)?;
        let width = 800u32.min(info.width);
        let height =
            ((width as f64 * info.height as f64 / info.width as f64).round() as u32).max(1);
        let output = self
            .command(false)
            .args(["-v", "error", "-ss", &format!("{seconds:.6}"), "-i"])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                &format!("scale={width}:{height}"),
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "pipe:1",
            ])
            .output()?;
        ensure!(
            output.status.success() && output.stdout.len() == width as usize * height as usize * 4,
            "Could not load preview frame."
        );
        Ok(eframe::egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &output.stdout,
        ))
    }
}

pub fn preview_dimensions(project: &EditProject) -> (u32, u32, u32) {
    render::dimensions(
        project,
        &ExportOptions {
            path: PathBuf::new(),
            height: Some(720),
            size_mb: None,
            canvas: None,
        },
    )
}
