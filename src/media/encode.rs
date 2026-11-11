//! Windows H.264/AAC sink writer shared by export and native test fixtures.
use super::{native::*, *};
use windows::{Win32::Media::MediaFoundation::*, core::PCWSTR};

pub(super) struct Writer {
    sink: IMFSinkWriter,
    pub video: u32,
    pub audio: Vec<u32>,
    _runtime: Runtime,
}
impl Writer {
    pub fn new(
        path: &Path,
        dimensions: (u32, u32, u32),
        bitrate: u64,
        audio_tracks: usize,
    ) -> Result<Self> {
        let runtime = Runtime::new()?;
        unsafe {
            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1)?;
            let attributes = attributes.unwrap();
            attributes.SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)?;
            let path = std::path::absolute(path)?;
            let sink = MFCreateSinkWriterFromURL(PCWSTR(wide(&path).as_ptr()), None, &attributes)?;
            let video = add_video(&sink, dimensions, bitrate)?;
            let audio = (0..audio_tracks)
                .map(|_| add_audio(&sink))
                .collect::<Result<_>>()?;
            sink.BeginWriting()?;
            Ok(Self {
                sink,
                video,
                audio,
                _runtime: runtime,
            })
        }
    }
    pub fn write(&self, stream: u32, bytes: &[u8], range: std::ops::Range<i64>) -> Result<()> {
        let sample = make_sample(bytes, range.start, range.end - range.start)?;
        unsafe {
            self.sink.WriteSample(stream, &sample)?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<()> {
        unsafe {
            self.sink.Finalize()?;
        }
        Ok(())
    }
}

fn add_video(sink: &IMFSinkWriter, dimensions: (u32, u32, u32), bitrate: u64) -> Result<u32> {
    let output = media_type(&MFMediaType_Video, &MFVideoFormat_H264)?;
    let input = media_type(&MFMediaType_Video, &MFVideoFormat_RGB32)?;
    let (width, height, fps) = dimensions;
    unsafe {
        for media in [&output, &input] {
            media.SetUINT64(&MF_MT_FRAME_SIZE, (width as u64) << 32 | height as u64)?;
            media.SetUINT64(&MF_MT_FRAME_RATE, (fps as u64) << 32 | 1)?;
            media.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, 1u64 << 32 | 1)?;
            media.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        }
        output.SetUINT32(&MF_MT_AVG_BITRATE, u32::try_from(bitrate)?)?;
        output.SetUINT32(&MF_MT_MPEG2_PROFILE, 77)?;
        // The raster renderer produces top-down BGRX, not a bottom-up DIB.
        input.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)?;
        let stream = sink.AddStream(&output)?;
        sink.SetInputMediaType(stream, &input, None)?;
        Ok(stream)
    }
}

fn add_audio(sink: &IMFSinkWriter) -> Result<u32> {
    let output = media_type(&MFMediaType_Audio, &MFAudioFormat_AAC)?;
    let input = media_type(&MFMediaType_Audio, &MFAudioFormat_PCM)?;
    unsafe {
        for media in [&output, &input] {
            media.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
            media.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
            media.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        }
        output.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000)?;
        output.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)?;
        input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
        input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000)?;
        let stream = sink.AddStream(&output)?;
        sink.SetInputMediaType(stream, &input, None)?;
        Ok(stream)
    }
}
