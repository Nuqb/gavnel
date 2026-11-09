use crate::replay::{Snapshot, Stream, TICKS};
use anyhow::{Context, Result, bail};
use bytes::Bytes;
use mp4::{AacConfig, AvcConfig, Mp4Config, Mp4Sample, Mp4Writer, TrackConfig};
use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

pub fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(70)
        .collect();
    if s.is_empty() { "Clip".into() } else { s }
}

pub fn write_clip(snapshot: &Snapshot, folder: &Path) -> Result<PathBuf> {
    let first_video = snapshot.packets.iter().find(|p| p.stream == Stream::Video);
    if !first_video.is_some_and(|p| p.keyframe) {
        bail!("A clip must begin on a video keyframe.");
    }
    let (file, temp_path, final_path) = create_destination(&snapshot.source, folder)?;
    let result = mux(snapshot, file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }
    std::fs::rename(&temp_path, &final_path)
        .context("Clip finalization failed; the .partial file was preserved")?;
    Ok(final_path)
}

pub(crate) fn create_destination(source: &str, folder: &Path) -> Result<(File, PathBuf, PathBuf)> {
    let folder = source_folder(folder, source)?;
    let base = format!(
        "{}_{}",
        safe_name(source),
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_%3f")
    );
    let mut selected = None;
    for suffix in 0..1000 {
        let name = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}_{suffix}")
        };
        let final_path = folder.join(format!("{name}.mp4"));
        if final_path.exists() {
            continue;
        }
        let temp_path = folder.join(format!("{name}.mp4.partial"));
        match OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                selected = Some((file, temp_path, final_path));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(e).context(
                    "Could not create a clip; check storage space and folder permissions",
                );
            }
        }
    }
    selected.ok_or_else(|| anyhow::anyhow!("Could not choose a unique clip filename"))
}

pub(crate) fn source_folder_name(source: &str) -> String {
    let source = source.trim();
    if source
        .strip_prefix("Display ")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        return "Desktop".into();
    }
    let name: String = source
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(70)
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    if name.is_empty() || name == "." || name == ".." {
        return "Other recordings".into();
    }
    let device = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$"].contains(&device.as_str())
        || (device.len() == 4
            && (device.starts_with("COM") || device.starts_with("LPT"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
    {
        format!("_{name}")
    } else {
        name.into()
    }
}

pub(crate) fn source_folder(root: &Path, source: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(root).context("Could not create the clips folder")?;
    let folder = root.join(source_folder_name(source));
    std::fs::create_dir_all(&folder).context("Could not create the game/source folder")?;
    if !folder.canonicalize()?.starts_with(root.canonicalize()?) {
        bail!("The game/source folder points outside your chosen storage location.");
    }
    Ok(folder)
}

fn mux(s: &Snapshot, file: File) -> Result<()> {
    let videos: Vec<_> = s
        .packets
        .iter()
        .filter(|p| p.stream == Stream::Video)
        .collect();
    if videos.is_empty() || !videos[0].keyframe {
        bail!("A clip must begin on a video keyframe.");
    }
    let config = Mp4Config {
        major_brand: "isom".parse()?,
        minor_version: 512,
        compatible_brands: vec![
            "isom".parse()?,
            "iso2".parse()?,
            "avc1".parse()?,
            "mp41".parse()?,
        ],
        timescale: 1000,
    };
    let mut writer = Mp4Writer::write_start(BufWriter::with_capacity(256 * 1024, file), &config)?;
    let mut track: TrackConfig = AvcConfig {
        width: s.video.width,
        height: s.video.height,
        seq_param_set: s.video.sps.clone(),
        pic_param_set: s.video.pps.clone(),
    }
    .into();
    track.timescale = 90_000;
    writer.add_track(&track)?;
    let audio: Vec<_> = s
        .packets
        .iter()
        .filter(|p| p.stream == Stream::Audio)
        .collect();
    if s.has_audio && !audio.is_empty() {
        let mut track: TrackConfig = AacConfig {
            bitrate: 192_000,
            ..Default::default()
        }
        .into();
        track.timescale = 48_000;
        writer.add_track(&track)?;
    }
    // Sample durations carry timeline gaps: the mp4 crate accumulates durations,
    // rather than using start_time to insert missing video frames.
    for (i, p) in videos.iter().enumerate() {
        let next = videos.get(i + 1).map(|n| n.pts).unwrap_or(s.end);
        let duration = ((next.saturating_sub(p.pts)) * 90_000 / TICKS).max(1) as u32;
        writer.write_sample(
            1,
            &Mp4Sample {
                start_time: (p.pts - s.start) * 90_000 / TICKS,
                duration,
                rendering_offset: 0,
                is_sync: p.keyframe,
                bytes: Bytes::copy_from_slice(&p.data.bytes),
            },
        )?;
    }
    if s.has_audio && !audio.is_empty() {
        for p in audio {
            writer.write_sample(
                2,
                &Mp4Sample {
                    start_time: (p.pts - s.start) * 48_000 / TICKS,
                    duration: 1024,
                    rendering_offset: 0,
                    is_sync: true,
                    bytes: Bytes::copy_from_slice(&p.data.bytes),
                },
            )?;
        }
    }
    let mut next_id = if s.has_audio && !s.packets.iter().all(|p| p.stream != Stream::Audio) {
        3
    } else {
        2
    };
    for stream in [Stream::Desktop, Stream::Microphone] {
        let packets: Vec<_> = s.packets.iter().filter(|p| p.stream == stream).collect();
        if packets.is_empty() {
            continue;
        }
        let mut track: TrackConfig = AacConfig {
            bitrate: 192_000,
            ..Default::default()
        }
        .into();
        track.timescale = 48_000;
        writer.add_track(&track)?;
        for p in packets {
            writer.write_sample(
                next_id,
                &Mp4Sample {
                    start_time: (p.pts - s.start) * 48_000 / TICKS,
                    duration: 1024,
                    rendering_offset: 0,
                    is_sync: true,
                    bytes: Bytes::copy_from_slice(&p.data.bytes),
                },
            )?;
        }
        next_id += 1;
    }
    writer.write_end()?;
    let mut buffer = writer.into_writer();
    buffer.flush()?;
    buffer.get_ref().sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::*;
    #[test]
    fn filename_cannot_escape_folder() {
        assert_eq!(safe_name("../game:name"), "___game_name");
    }
    #[test]
    fn storage_groups_sources_and_handles_windows_folder_names() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(source_folder_name("Display 2"), "Desktop");
        assert_eq!(source_folder_name("Counter-Strike 2"), "Counter-Strike 2");
        assert_eq!(source_folder_name("../a:b?"), ".._a_b_");
        assert_eq!(source_folder_name("CON.txt"), "_CON.txt");
        assert_eq!(source_folder_name("LPT1"), "_LPT1");
        assert_eq!(source_folder_name(" .  "), "Other recordings");
        assert_eq!(source_folder_name("Fortnite... "), "Fortnite");
        for source in ["Fortnite", "Fortnite", "Display 1", "../a:b?"] {
            let (file, partial, final_path) = create_destination(source, root.path()).unwrap();
            assert!(partial.starts_with(root.path()));
            assert_eq!(
                partial.parent(),
                Some(root.path().join(source_folder_name(source)).as_path())
            );
            drop(file);
            std::fs::rename(partial, final_path).unwrap();
        }
        assert_eq!(
            std::fs::read_dir(root.path().join("Fortnite"))
                .unwrap()
                .count(),
            2
        );
    }
    #[test]
    fn failure_does_not_publish_partial_clip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Snapshot {
            source: "test".into(),
            video: VideoConfig {
                width: 1920,
                height: 1080,
                sps: vec![],
                pps: vec![],
            },
            packets: vec![],
            start: 0,
            end: 0,
            has_audio: false,
        };
        assert!(write_clip(&s, dir.path()).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[test]
    fn writes_parseable_h264_container() {
        let dir = tempfile::tempdir().unwrap();
        let budget = Budget::new(1024);
        let s = Snapshot {
            source: "Test".into(),
            video: VideoConfig {
                width: 1920,
                height: 1080,
                sps: vec![0x67, 0x64, 0, 0x28, 0xac, 0xd9],
                pps: vec![0x68, 0xee, 0x3c, 0x80],
            },
            packets: vec![Packet {
                stream: Stream::Video,
                pts: 0,
                duration: TICKS / 60,
                keyframe: true,
                data: budget.allocate(vec![0, 0, 0, 2, 0x65, 0x88]).unwrap(),
            }],
            start: 0,
            end: TICKS / 60,
            has_audio: false,
        };
        let path = write_clip(&s, dir.path()).unwrap();
        let file = File::open(&path).unwrap();
        let parsed = mp4::Mp4Reader::read_header(
            std::io::BufReader::new(file),
            std::fs::metadata(path).unwrap().len(),
        )
        .unwrap();
        assert_eq!(parsed.tracks().len(), 1);
        assert_eq!(parsed.sample_count(1).unwrap(), 1);
    }

    #[test]
    fn clip_preserves_separate_audio_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let budget = Budget::new(4096);
        let packets = [
            Stream::Video,
            Stream::Audio,
            Stream::Desktop,
            Stream::Microphone,
        ]
        .into_iter()
        .map(|stream| Packet {
            stream,
            pts: 0,
            duration: TICKS / 60,
            keyframe: true,
            data: budget.allocate(vec![0, 0, 0, 2, 0x65, 0x88]).unwrap(),
        })
        .collect();
        let snapshot = Snapshot {
            source: "Audio tracks".into(),
            video: VideoConfig {
                width: 1920,
                height: 1080,
                sps: vec![0x67, 0x64, 0, 0x28, 0xac, 0xd9],
                pps: vec![0x68, 0xee, 0x3c, 0x80],
            },
            packets,
            start: 0,
            end: TICKS / 60,
            has_audio: true,
        };
        let path = write_clip(&snapshot, dir.path()).unwrap();
        let reader = mp4::Mp4Reader::read_header(
            std::io::BufReader::new(File::open(&path).unwrap()),
            path.metadata().unwrap().len(),
        )
        .unwrap();
        assert_eq!(reader.tracks().len(), 4);
        for id in 1..=4 {
            assert_eq!(reader.sample_count(id).unwrap(), 1);
        }
    }
}
