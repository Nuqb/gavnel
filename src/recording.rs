//! Continuous capture streams encoded packets to disk independently of replay retention.
use crate::{
    export::create_destination,
    replay::{Packet, Stream, TICKS, VideoConfig},
};
use anyhow::{Context, Result, bail};
use bytes::Bytes;
use crossbeam_channel::{Sender, bounded};
use mp4::{AacConfig, AvcConfig, Mp4Config, Mp4Sample, Mp4Writer, TrackConfig};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    thread::JoinHandle,
};

pub struct SessionWriter {
    path: PathBuf,
    start: std::cell::Cell<Option<u64>>,
    packets: Option<Sender<Packet>>,
    worker: Option<JoinHandle<Result<PathBuf>>>,
}

impl SessionWriter {
    #[cfg(test)]
    pub fn start(source: &str, folder: &Path, video: &VideoConfig, audio: bool) -> Result<Self> {
        Self::start_limited(source, folder, video, audio, None)
    }
    pub fn start_limited(
        source: &str,
        folder: &Path,
        video: &VideoConfig,
        audio: bool,
        limit: Option<u32>,
    ) -> Result<Self> {
        crate::storage::check_limit(folder, limit, 0)?;
        let folder = folder.to_owned();
        // Open before starting capture so permission/storage failures are immediate.
        let mux = ContinuousMp4::new(source, &folder, video, audio)?;
        let path = mux.final_path.clone();
        let (tx, rx) = bounded::<Packet>(512);
        let worker = std::thread::Builder::new()
            .name("gavnel-recording-writer".into())
            .spawn(move || {
                let mut mux = mux;
                let mut checked = std::time::Instant::now();
                for packet in rx {
                    if limit.is_some() && checked.elapsed() >= std::time::Duration::from_secs(1) {
                        checked = std::time::Instant::now();
                        if crate::storage::check_limit(&folder, limit, 0).is_err() {
                            return mux.finish();
                        }
                    }
                    if let Err(error) = mux.push(packet) {
                        bail!(
                            "Recording write failed: {error:#}. Unfinished file: {}",
                            mux.temp_path.display()
                        );
                    }
                }
                mux.finish()
            })?;
        Ok(Self {
            path,
            start: std::cell::Cell::new(None),
            packets: Some(tx),
            worker: Some(worker),
        })
    }

    pub fn bookmark(&self, ticks: u64) -> Result<f64> {
        let start = self
            .start
            .get()
            .context("Wait for recording to begin before bookmarking")?;
        let seconds = ticks.saturating_sub(start) as f64 / TICKS as f64;
        crate::bookmarks::append(&self.path, seconds)?;
        Ok(seconds)
    }

    pub fn push(&self, packet: Packet) -> Result<()> {
        if self.start.get().is_none() && packet.stream == Stream::Video && packet.keyframe {
            self.start.set(Some(packet.pts));
        }
        self.packets.as_ref().context("Recording writer is closed")?.try_send(packet)
            .map_err(|_| anyhow::anyhow!("Recording storage could not keep up or became unavailable. Stopping and saving received frames."))
    }

    pub fn finish(mut self) -> Result<PathBuf> {
        self.packets.take();
        self.worker
            .take()
            .context("Recording writer is closed")?
            .join()
            .map_err(|_| anyhow::anyhow!("Recording writer stopped unexpectedly"))?
    }

    pub fn push_final(&self, packet: Packet) -> Result<()> {
        self.packets
            .as_ref()
            .context("Recording writer is closed")?
            .send_timeout(packet, std::time::Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("Recording storage did not accept the final frames"))
    }
}

impl Drop for SessionWriter {
    fn drop(&mut self) {
        self.packets.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct ContinuousMp4 {
    writer: Mp4Writer<BufWriter<File>>,
    temp_path: PathBuf,
    final_path: PathBuf,
    start: Option<u64>,
    pending_video: Option<Packet>,
    audio: bool,
    last_audio: Option<u64>,
    extra_audio: Vec<(Stream, u32, Option<u64>)>,
}

impl ContinuousMp4 {
    fn new(source: &str, folder: &Path, video: &VideoConfig, audio: bool) -> Result<Self> {
        let (file, temp_path, final_path) = create_destination(source, folder)?;
        let result = (|| -> Result<_> {
            let mut writer = Mp4Writer::write_start(
                BufWriter::with_capacity(256 * 1024, file),
                &Mp4Config {
                    major_brand: "isom".parse()?,
                    minor_version: 512,
                    compatible_brands: vec![
                        "isom".parse()?,
                        "iso2".parse()?,
                        "avc1".parse()?,
                        "mp41".parse()?,
                    ],
                    // Divisible by both track timescales; avoids per-sample movie duration rounding.
                    timescale: 720_000,
                },
            )?;
            let mut track: TrackConfig = AvcConfig {
                width: video.width,
                height: video.height,
                seq_param_set: video.sps.clone(),
                pic_param_set: video.pps.clone(),
            }
            .into();
            track.timescale = 90_000;
            writer.add_track(&track)?;
            if audio {
                let mut track: TrackConfig = AacConfig {
                    bitrate: 192_000,
                    ..Default::default()
                }
                .into();
                track.timescale = 48_000;
                writer.add_track(&track)?;
            }
            Ok(writer)
        })();
        let writer = match result {
            Ok(writer) => writer,
            Err(error) => {
                let _ = std::fs::remove_file(&temp_path);
                return Err(error);
            }
        };
        Ok(Self {
            writer,
            temp_path,
            final_path,
            start: None,
            pending_video: None,
            audio,
            last_audio: None,
            extra_audio: Vec::new(),
        })
    }

    fn push(&mut self, packet: Packet) -> Result<()> {
        if self.start.is_none() {
            if packet.stream != Stream::Video || !packet.keyframe {
                return Ok(());
            }
            self.start = Some(packet.pts);
        }
        let start = self.start.unwrap();
        if packet.pts < start {
            return Ok(());
        }
        match packet.stream {
            Stream::Video => {
                if self
                    .pending_video
                    .as_ref()
                    .is_some_and(|p| packet.pts <= p.pts)
                {
                    return Ok(());
                }
                if let Some(previous) = self.pending_video.take() {
                    self.write_video(&previous, packet.pts)?;
                }
                self.pending_video = Some(packet);
            }
            Stream::Audio if self.audio => {
                if self.last_audio.is_some_and(|last| packet.pts <= last) {
                    return Ok(());
                }
                self.writer.write_sample(
                    2,
                    &Mp4Sample {
                        start_time: (packet.pts - start) * 48_000 / TICKS,
                        duration: 1024,
                        rendering_offset: 0,
                        is_sync: true,
                        bytes: Bytes::copy_from_slice(&packet.data.bytes),
                    },
                )?;
                self.last_audio = Some(packet.pts);
            }
            Stream::Desktop | Stream::Microphone if self.audio => {
                let index = if let Some(index) = self
                    .extra_audio
                    .iter()
                    .position(|(stream, _, _)| *stream == packet.stream)
                {
                    index
                } else {
                    let mut track: TrackConfig = AacConfig {
                        bitrate: 192_000,
                        ..Default::default()
                    }
                    .into();
                    track.timescale = 48_000;
                    self.writer.add_track(&track)?;
                    let index = self.extra_audio.len();
                    self.extra_audio
                        .push((packet.stream, 3 + index as u32, None));
                    index
                };
                let (_, id, last) = &mut self.extra_audio[index];
                if last.is_some_and(|pts| packet.pts <= pts) {
                    return Ok(());
                }
                self.writer.write_sample(
                    *id,
                    &Mp4Sample {
                        start_time: (packet.pts - start) * 48_000 / TICKS,
                        duration: 1024,
                        rendering_offset: 0,
                        is_sync: true,
                        bytes: Bytes::copy_from_slice(&packet.data.bytes),
                    },
                )?;
                *last = Some(packet.pts);
            }
            _ => {}
        }
        Ok(())
    }

    fn write_video(&mut self, packet: &Packet, end: u64) -> Result<()> {
        let start = self.start.unwrap();
        // Scale absolute timestamps before subtraction to avoid hours of rounding drift.
        let time = (packet.pts - start) * 90_000 / TICKS;
        let end = (end - start) * 90_000 / TICKS;
        let duration = u32::try_from(end.saturating_sub(time).max(1))
            .context("Video timing gap is too large")?;
        self.writer.write_sample(
            1,
            &Mp4Sample {
                start_time: time,
                duration,
                rendering_offset: 0,
                is_sync: packet.keyframe,
                bytes: Bytes::copy_from_slice(&packet.data.bytes),
            },
        )?;
        Ok(())
    }

    fn finish(mut self) -> Result<PathBuf> {
        if let Some(packet) = self.pending_video.take() {
            self.write_video(&packet, packet.pts + packet.duration)?;
        } else {
            drop(self.writer);
            let _ = std::fs::remove_file(&self.temp_path);
            bail!("Recording stopped before any video frames arrived. No file was saved.");
        }
        let result = (|| -> Result<()> {
            self.writer.write_end()?;
            let mut file = self.writer.into_writer();
            file.flush()?;
            file.get_ref().sync_all()?;
            drop(file);
            std::fs::rename(&self.temp_path, &self.final_path)?;
            Ok(())
        })();
        result.with_context(|| {
            format!(
                "Recording could not be finalized; unfinished file preserved at {}",
                self.temp_path.display()
            )
        })?;
        Ok(self.final_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::Budget;

    fn config() -> VideoConfig {
        VideoConfig {
            width: 1920,
            height: 1080,
            sps: vec![0x67, 0x64, 0, 0x28, 0xac, 0xd9],
            pps: vec![0x68, 0xee, 0x3c, 0x80],
        }
    }
    fn packet(budget: &std::sync::Arc<Budget>, pts: u64, keyframe: bool) -> Packet {
        Packet {
            stream: Stream::Video,
            pts,
            duration: TICKS / 60,
            keyframe,
            data: budget.allocate(vec![0, 0, 0, 2, 0x65, 0x88]).unwrap(),
        }
    }

    #[test]
    fn streams_beyond_replay_length_without_retaining_payloads_or_timing_drift() {
        let dir = tempfile::tempdir().unwrap();
        let budget = Budget::new(1024);
        let mut mux = ContinuousMp4::new("Session", dir.path(), &config(), false).unwrap();
        for frame in 0..(60 * 180) {
            mux.push(packet(&budget, frame * TICKS / 60, frame % 120 == 0))
                .unwrap();
            assert_eq!(budget.used(), 6); // Only the pending video frame remains.
        }
        let path = mux.finish().unwrap();
        assert_eq!(budget.used(), 0);
        let reader = mp4::Mp4Reader::read_header(
            std::io::BufReader::new(File::open(&path).unwrap()),
            path.metadata().unwrap().len(),
        )
        .unwrap();
        assert_eq!(reader.sample_count(1).unwrap(), 10_800);
        assert!(
            (reader.duration().as_secs_f64() - 180.0).abs() < 0.002,
            "duration {:?}",
            reader.duration()
        );
    }

    #[test]
    fn waits_for_keyframe_preserves_gaps_and_cleans_empty_recording() {
        let dir = tempfile::tempdir().unwrap();
        let budget = Budget::new(1024);
        let mut mux = ContinuousMp4::new("Empty", dir.path(), &config(), false).unwrap();
        mux.push(packet(&budget, 0, false)).unwrap();
        assert!(mux.finish().is_err());
        assert_eq!(
            std::fs::read_dir(dir.path().join("Empty")).unwrap().count(),
            0
        );
        let mut mux = ContinuousMp4::new("Gap", dir.path(), &config(), false).unwrap();
        mux.push(packet(&budget, 5 * TICKS, true)).unwrap();
        mux.push(packet(&budget, 7 * TICKS, false)).unwrap();
        let path = mux.finish().unwrap();
        let mut reader = mp4::Mp4Reader::read_header(
            std::io::BufReader::new(File::open(&path).unwrap()),
            path.metadata().unwrap().len(),
        )
        .unwrap();
        assert_eq!(reader.read_sample(1, 1).unwrap().unwrap().duration, 180_000);
    }

    #[test]
    fn bookmarks_use_the_video_origin_and_survive_finalization() {
        let dir = tempfile::tempdir().unwrap();
        let budget = Budget::new(16384);
        let writer = SessionWriter::start("Bookmarks", dir.path(), &config(), false).unwrap();
        assert!(writer.bookmark(TICKS).is_err());
        writer.push(packet(&budget, 5 * TICKS, false)).unwrap();
        assert!(writer.bookmark(6 * TICKS).is_err());
        writer.push(packet(&budget, 7 * TICKS, true)).unwrap();
        writer.push(packet(&budget, 10 * TICKS, false)).unwrap();
        assert_eq!(writer.bookmark(9 * TICKS).unwrap(), 2.0);
        let path = writer.finish().unwrap();
        assert_eq!(crate::bookmarks::load(&path).unwrap(), vec![2.0]);
    }

    #[test]
    fn worker_drains_queued_audio_and_video_before_publishing() {
        let dir = tempfile::tempdir().unwrap();
        let budget = Budget::new(16384);
        let writer = SessionWriter::start("Session", dir.path(), &config(), true).unwrap();
        assert!(
            std::fs::read_dir(dir.path().join("Session"))
                .unwrap()
                .all(|e| e.unwrap().path().extension().unwrap() == "partial")
        );
        for frame in 0..60 {
            writer
                .push(packet(&budget, frame * TICKS / 60, frame == 0))
                .unwrap();
        }
        for index in 0..46 {
            for stream in [Stream::Audio, Stream::Desktop, Stream::Microphone] {
                let mut audio = packet(&budget, index * 1024 * TICKS / 48_000, true);
                audio.stream = stream;
                writer.push(audio).unwrap();
            }
        }
        let path = writer.finish().unwrap();
        assert_eq!(budget.used(), 0);
        let reader = mp4::Mp4Reader::read_header(
            std::io::BufReader::new(File::open(&path).unwrap()),
            path.metadata().unwrap().len(),
        )
        .unwrap();
        assert_eq!(reader.sample_count(1).unwrap(), 60);
        assert_eq!(reader.sample_count(2).unwrap(), 46);
        assert_eq!(reader.sample_count(3).unwrap(), 46);
        assert_eq!(reader.sample_count(4).unwrap(), 46);
        assert_eq!(
            std::fs::read_dir(dir.path().join("Session"))
                .unwrap()
                .count(),
            1
        );
    }
}
