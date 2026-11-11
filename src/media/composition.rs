//! Stream frames and mixed PCM through the Windows encoder with bounded memory.
use super::{audio::Mixer, encode::Writer, frames::Video, native::TICKS, *};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct Render<'a> {
    pub project: &'a EditProject,
    pub dimensions: (u32, u32, u32),
    pub bitrate: u64,
    pub progress: &'a Arc<Mutex<JobProgress>>,
    pub cancel: &'a AtomicBool,
}
impl Render<'_> {
    pub fn write(&self, path: &Path) -> Result<()> {
        self.check_cancel()?;
        let writer = Writer::new(path, self.dimensions, self.bitrate, 1)
            .context("Windows could not initialize H.264/AAC export.")?;
        let mut elapsed = 0.0;
        for (index, segment) in self.project.segments.iter().enumerate() {
            self.progress.lock().unwrap().message = format!(
                "Exporting clip {} of {}",
                index + 1,
                self.project.segments.len()
            );
            self.segment(&writer, segment, elapsed)?;
            elapsed += segment.duration();
        }
        self.check_cancel()?;
        self.progress.lock().unwrap().message = "Finishing export".into();
        writer.finish()
    }
    fn segment(&self, writer: &Writer, segment: &ClipSegment, elapsed: f64) -> Result<()> {
        let mut video = Video::new(&segment.info)?;
        video.seek(segment.start)?;
        let mut mixer = Mixer::new(segment)?;
        let (width, height, fps) = self.dimensions;
        let frames = (segment.duration() * fps as f64 - 1e-7).ceil() as u64;
        let mut audio_offset = 0;
        let audio_origin = (elapsed * 48_000.0).round() as u64;
        for frame in 0..frames {
            self.check_cancel()?;
            let begin = frame as f64 / fps as f64;
            let end = ((frame + 1) as f64 / fps as f64).min(segment.duration());
            let raster = video.at(segment.start + begin)?;
            let pixels = raster.render(segment.crop.pixels(&segment.info), (width, height));
            let range = ((elapsed + begin) * TICKS).round() as i64
                ..((elapsed + end) * TICKS).round() as i64;
            writer.write(writer.video, &pixels, range)?;
            let audio_end = ((elapsed + end) * 48_000.0).round() as u64 - audio_origin;
            while audio_offset < audio_end {
                self.check_cancel()?;
                let count = (audio_end - audio_offset).min(1024) as usize;
                let pcm = mixer.pcm(audio_offset, count)?;
                let start = audio_origin + audio_offset;
                let range = (start as f64 * TICKS / 48_000.0).round() as i64
                    ..((start + count as u64) as f64 * TICKS / 48_000.0).round() as i64;
                writer.write(writer.audio[0], &pcm, range)?;
                audio_offset += count as u64;
            }
            self.progress.lock().unwrap().fraction =
                ((elapsed + end) / self.project.duration() * 0.99) as f32;
        }
        Ok(())
    }
    fn check_cancel(&self) -> Result<()> {
        ensure!(!self.cancel.load(Ordering::Acquire), "Cancelled");
        Ok(())
    }
}
