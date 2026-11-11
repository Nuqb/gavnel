//! Bounded, timestamp-aligned decoding and mixing of the selected audio tracks.
use super::{native::*, *};
use windows::Win32::Media::MediaFoundation::*;

struct Track {
    source: Source,
    index: u32,
    gain: f32,
    start: i64,
    frames: Vec<[f32; 2]>,
    ended: bool,
}
impl Track {
    fn open(segment: &ClipSegment, track: &TrackMix) -> Result<Self> {
        let source = Source::open(&segment.info.path)?;
        let media = media_type(&MFMediaType_Audio, &MFAudioFormat_Float)?;
        unsafe {
            media.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            media.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
            media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 32)?;
        }
        source.select(track.index, &media)?;
        source.seek(segment.start)?;
        Ok(Self {
            source,
            index: track.index,
            gain: segment.gain(track) as f32,
            start: 0,
            frames: Vec::new(),
            ended: false,
        })
    }
    fn advance(&mut self) -> Result<()> {
        match self.source.read(self.index)? {
            Some((time, sample)) => {
                let media = unsafe { self.source.reader.GetCurrentMediaType(self.index)? };
                ensure!(
                    unsafe { media.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)? } == 48_000
                        && unsafe { media.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)? } == 2
                        && unsafe { media.GetGUID(&MF_MT_SUBTYPE)? } == MFAudioFormat_Float,
                    "Audio format changed during decoding."
                );
                let bytes = sample_bytes(&sample)?;
                ensure!(bytes.len() % 8 == 0, "Invalid decoded audio sample.");
                self.start = (time as f64 * 48_000.0 / TICKS).round() as i64;
                self.frames = bytes
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|bytes| {
                        [
                            f32::from_le_bytes(bytes[..4].try_into().unwrap()),
                            f32::from_le_bytes(bytes[4..].try_into().unwrap()),
                        ]
                    })
                    .collect();
            }
            None => {
                self.ended = true;
                self.frames.clear();
            }
        }
        Ok(())
    }
    fn mix(&mut self, start: i64, output: &mut [[f32; 2]]) -> Result<()> {
        let end = start + output.len() as i64;
        loop {
            let chunk_end = self.start + self.frames.len() as i64;
            if chunk_end <= start || self.frames.is_empty() {
                if self.ended {
                    break;
                }
                self.advance()?;
                continue;
            }
            let first = start.max(self.start);
            let last = end.min(chunk_end);
            for frame in first..last {
                for (target, value) in output[(frame - start) as usize]
                    .iter_mut()
                    .zip(self.frames[(frame - self.start) as usize])
                {
                    if value.is_finite() {
                        *target += value * self.gain;
                    }
                }
            }
            if chunk_end >= end || self.ended {
                break;
            }
            self.advance()?;
        }
        Ok(())
    }
}

pub(super) struct Mixer {
    tracks: Vec<Track>,
    origin: i64,
    level: f32,
}
impl Mixer {
    pub fn new(segment: &ClipSegment) -> Result<Self> {
        let tracks = segment
            .audio
            .iter()
            .filter(|track| segment.gain(track) > 0.0)
            .map(|track| Track::open(segment, track))
            .collect::<Result<_>>()?;
        Ok(Self {
            tracks,
            origin: (segment.start * 48_000.0).round() as i64,
            level: 1.0,
        })
    }
    pub fn pcm(&mut self, offset: u64, count: usize) -> Result<Vec<u8>> {
        let mut mixed = vec![[0.0; 2]; count];
        for track in &mut self.tracks {
            track.mix(self.origin + offset as i64, &mut mixed)?;
        }
        let mut pcm = Vec::with_capacity(count * 4);
        for frame in mixed {
            let peak = frame[0].abs().max(frame[1].abs()).max(0.98);
            // Stereo-linked peak limiting with immediate attack and a 50 ms
            // release avoids hard-clipping summed tracks and preserves balance.
            self.level = (0.98 / peak).min(self.level + (1.0 - self.level) * 0.00041658);
            for value in frame {
                pcm.extend_from_slice(
                    &((value * self.level * 32767.0).round() as i16).to_le_bytes(),
                );
            }
        }
        Ok(pcm)
    }
}
