use anyhow::{Result, bail};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

pub const TICKS: u64 = 10_000_000;
pub const MEMORY_LIMIT: usize = 512 * 1024 * 1024;

/// Output channels and memory accounting shared by the audio/video encoders.
pub struct EncoderOutput {
    pub budget: Arc<Budget>,
    pub packets: crossbeam_channel::Sender<Packet>,
    pub messages: crossbeam_channel::Sender<String>,
}

#[derive(Debug)]
pub struct Budget {
    used: AtomicUsize,
    limit: usize,
}
impl Budget {
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            used: AtomicUsize::new(0),
            limit,
        })
    }
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }
    pub fn allocate(self: &Arc<Self>, bytes: Vec<u8>) -> Option<Arc<Payload>> {
        let len = bytes.len();
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |used| {
                used.checked_add(len).filter(|n| *n <= self.limit)
            })
            .ok()?;
        Some(Arc::new(Payload {
            bytes,
            budget: self.clone(),
        }))
    }
}
#[derive(Debug)]
pub struct Payload {
    pub bytes: Vec<u8>,
    budget: Arc<Budget>,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.budget
            .used
            .fetch_sub(self.bytes.len(), Ordering::AcqRel);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Video,
    Audio,
    Desktop,
    Microphone,
}
#[derive(Clone, Debug)]
pub struct Packet {
    pub stream: Stream,
    pub pts: u64,
    pub duration: u64,
    pub keyframe: bool,
    pub data: Arc<Payload>,
}
#[derive(Clone, Debug)]
pub struct VideoConfig {
    pub width: u16,
    pub height: u16,
    pub sps: Vec<u8>,
    pub pps: Vec<u8>,
}
#[derive(Clone)]
pub struct Snapshot {
    pub source: String,
    pub video: VideoConfig,
    pub packets: Vec<Packet>,
    pub start: u64,
    pub end: u64,
    pub has_audio: bool,
}
pub struct ReplayBuffer {
    packets: VecDeque<Packet>,
    seconds: u64,
    pub video: Option<VideoConfig>,
    pub has_audio: bool,
}
impl ReplayBuffer {
    pub fn set_seconds(&mut self, seconds: u32) {
        self.seconds = u64::from(seconds);
    }
    pub fn new(seconds: u32, has_audio: bool) -> Self {
        Self {
            packets: VecDeque::new(),
            seconds: seconds as u64,
            video: None,
            has_audio,
        }
    }
    pub fn push(&mut self, packet: Packet) {
        if self
            .packets
            .back()
            .is_some_and(|p| p.stream == packet.stream && p.pts > packet.pts)
        {
            return;
        }
        self.packets.push_back(packet);
        let latest = self
            .packets
            .iter()
            .rev()
            .find(|p| p.stream == Stream::Video)
            .map(|p| p.pts)
            .unwrap_or(0);
        let cutoff = latest.saturating_sub(self.seconds * TICKS);
        if let Some(start) = self
            .packets
            .iter()
            .filter(|p| p.stream == Stream::Video && p.keyframe && p.pts <= cutoff)
            .map(|p| p.pts)
            .max()
        {
            self.packets.retain(|p| p.pts >= start);
        }
    }
    pub fn seconds(&self) -> f64 {
        let start = self
            .packets
            .iter()
            .find(|p| p.stream == Stream::Video && p.keyframe)
            .map(|p| p.pts);
        let video_end = self
            .packets
            .iter()
            .rev()
            .find(|p| p.stream == Stream::Video)
            .map(|p| p.pts + p.duration);
        match (start, video_end) {
            (Some(s), Some(e)) => e.saturating_sub(s) as f64 / TICKS as f64,
            _ => 0.0,
        }
    }
    /// Keep complete GOPs when a continuous recording needs replay-memory headroom.
    pub fn trim_bytes(&mut self, limit: usize) {
        let mut bytes: usize = self.packets.iter().map(|p| p.data.bytes.len()).sum();
        while bytes > limit {
            let first = self
                .packets
                .iter()
                .find(|p| p.stream == Stream::Video && p.keyframe)
                .map(|p| p.pts);
            let next = self
                .packets
                .iter()
                .find(|p| p.stream == Stream::Video && p.keyframe && Some(p.pts) > first)
                .map(|p| p.pts);
            let Some(next) = next else {
                break;
            };
            self.packets.retain(|p| {
                if p.pts < next {
                    bytes -= p.data.bytes.len();
                    false
                } else {
                    true
                }
            });
        }
    }
    #[cfg(test)]
    pub fn snapshot(&self, source: &str) -> Result<Snapshot> {
        self.snapshot_for(source, self.seconds as u32)
    }
    pub fn snapshot_for(&self, source: &str, seconds: u32) -> Result<Snapshot> {
        let video = self
            .video
            .clone()
            .ok_or_else(|| anyhow::anyhow!("The video encoder is still starting."))?;
        let video_end = self
            .packets
            .iter()
            .rev()
            .find(|p| p.stream == Stream::Video)
            .map(|p| p.pts + p.duration)
            .unwrap_or(0);
        // Audio mixing/encoding is deliberately asynchronous. End the saved
        // clip at the newest timestamp available in both streams so a save
        // cannot append a silent video tail while audio is catching up.
        let end = if self.has_audio {
            self.packets
                .iter()
                .rev()
                .find(|p| p.stream == Stream::Audio)
                .map(|p| video_end.min(p.pts + p.duration))
                .unwrap_or(video_end)
        } else {
            video_end
        };
        let cutoff = end.saturating_sub(u64::from(seconds).min(self.seconds) * TICKS);
        let start = self
            .packets
            .iter()
            .filter(|p| p.stream == Stream::Video && p.keyframe && p.pts <= cutoff)
            .map(|p| p.pts)
            .max()
            .or_else(|| {
                self.packets
                    .iter()
                    .find(|p| p.stream == Stream::Video && p.keyframe)
                    .map(|p| p.pts)
            });
        let Some(start) = start else {
            bail!("No decodable video is buffered yet. Try again in a moment.");
        };
        let mut packets: Vec<_> = self
            .packets
            .iter()
            .filter(|p| p.pts >= start && p.pts < end)
            .cloned()
            .collect();
        packets.sort_by_key(|p| p.pts);
        Ok(Snapshot {
            source: source.into(),
            video,
            packets,
            start,
            end,
            has_audio: self.has_audio,
        })
    }
}

pub fn annex_b_nals(bytes: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = if bytes[i..].starts_with(&[0, 0, 0, 1]) {
            4
        } else if bytes[i..].starts_with(&[0, 0, 1]) {
            3
        } else {
            0
        };
        if n > 0 {
            starts.push((i, i + n));
            i += n;
        } else {
            i += 1;
        }
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(i, (_, start))| {
            let end = starts.get(i + 1).map(|s| s.0).unwrap_or(bytes.len());
            (*start < end).then_some(&bytes[*start..end])
        })
        .collect()
}
pub fn avcc(bytes: &[u8]) -> Result<Vec<u8>> {
    let nals = annex_b_nals(bytes);
    if nals.is_empty() {
        bail!("NVENC returned a malformed H.264 packet.");
    }
    let mut out = Vec::with_capacity(bytes.len() + 16);
    for nal in nals {
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(b: &Arc<Budget>, t: u64, key: bool) -> Packet {
        Packet {
            stream: Stream::Video,
            pts: t * TICKS,
            duration: TICKS,
            keyframe: key,
            data: b.allocate(vec![1; 4]).unwrap(),
        }
    }
    fn config() -> VideoConfig {
        VideoConfig {
            width: 1920,
            height: 1080,
            sps: vec![0x67, 0x64, 0, 0x28],
            pps: vec![0x68, 0],
        }
    }
    #[test]
    fn buffer_starts_at_keyframe_and_evicts() {
        let b = Budget::new(10000);
        let mut r = ReplayBuffer::new(3, false);
        r.video = Some(config());
        for i in 0..10 {
            r.push(packet(&b, i, i % 2 == 0));
        }
        let s = r.snapshot("test").unwrap();
        assert_eq!(s.start, 6 * TICKS);
        assert_eq!(s.end, 10 * TICKS);
        assert!(s.packets[0].keyframe);
        assert!(r.packets.len() <= 6);
    }
    #[test]
    fn different_clip_hotkeys_use_their_own_lengths_from_one_buffer() {
        let budget = Budget::new(10000);
        let mut replay = ReplayBuffer::new(120, false);
        replay.video = Some(config());
        for second in 0..130 {
            replay.push(packet(&budget, second, second % 2 == 0));
        }
        let short = replay.snapshot_for("Game", 15).unwrap();
        let long = replay.snapshot_for("Game", 60).unwrap();
        assert_eq!(short.end, long.end);
        assert_eq!(short.end - short.start, 16 * TICKS);
        assert_eq!(long.end - long.start, 60 * TICKS);
        replay.set_seconds(30);
        replay.push(packet(&budget, 130, true));
        assert!(
            replay.snapshot_for("Game", 120).unwrap().end
                - replay.snapshot_for("Game", 120).unwrap().start
                <= 32 * TICKS
        );
    }
    #[test]
    fn shared_exports_count_memory_once() {
        let b = Budget::new(8);
        let p = b.allocate(vec![1; 8]).unwrap();
        let retained = p.clone();
        assert!(b.allocate(vec![2]).is_none());
        drop(p);
        assert_eq!(b.used(), 8);
        drop(retained);
        assert_eq!(b.used(), 0);
    }
    #[test]
    fn memory_trimming_retains_decodable_recent_video() {
        let b = Budget::new(1000);
        let mut replay = ReplayBuffer::new(120, false);
        replay.video = Some(config());
        for t in 0..10 {
            replay.push(packet(&b, t, t % 2 == 0));
        }
        replay.trim_bytes(12);
        let saved = replay.snapshot("test").unwrap();
        assert_eq!(saved.start, 8 * TICKS);
        assert_eq!(saved.end, 10 * TICKS);
        assert_eq!(b.used(), 8);
    }
    #[test]
    fn export_ends_at_the_common_audio_video_boundary() {
        let budget = Budget::new(1000);
        let mut replay = ReplayBuffer::new(60, true);
        replay.video = Some(config());
        replay.push(packet(&budget, 0, true));
        replay.push(packet(&budget, 1, false));
        replay.push(Packet {
            stream: Stream::Audio,
            pts: TICKS,
            duration: TICKS / 2,
            keyframe: true,
            data: budget.allocate(vec![2; 4]).unwrap(),
        });
        replay.push(packet(&budget, 2, true));
        let snapshot = replay.snapshot("test").unwrap();
        assert_eq!(snapshot.end, TICKS + TICKS / 2);
        assert!(snapshot.packets.iter().all(|p| p.pts < snapshot.end));
    }
    #[test]
    fn short_buffer_and_missing_keyframe() {
        let b = Budget::new(100);
        let mut r = ReplayBuffer::new(60, false);
        r.video = Some(config());
        r.push(packet(&b, 0, false));
        assert!(r.snapshot("x").is_err());
        r.push(packet(&b, 1, true));
        assert_eq!(r.snapshot("x").unwrap().start, TICKS);
    }
    #[test]
    fn mixed_annex_b_start_codes() {
        let b = [0, 0, 0, 1, 0x67, 2, 0, 0, 1, 0x68, 3];
        assert_eq!(annex_b_nals(&b), vec![&[0x67, 2][..], &[0x68, 3][..]]);
        assert_eq!(
            avcc(&b).unwrap(),
            vec![0, 0, 0, 2, 0x67, 2, 0, 0, 0, 2, 0x68, 3]
        );
        assert!(avcc(&[1, 2, 3]).is_err());
    }
}
