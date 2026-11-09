//! Media Foundation AAC transform and packet production.
use super::*;

struct MediaFoundation;

impl MediaFoundation {
    fn new() -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        }
        Ok(Self)
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

pub(super) struct AacEncoder {
    pub(super) transform: IMFTransform,
    pub(super) output_size: u32,
    pub(super) output_index: u64,
    pub(super) stream: Stream,
    // Declared last so the transform is released before Media Foundation stops.
    _runtime: MediaFoundation,
}
impl AacEncoder {
    pub(super) fn new() -> Result<Self> {
        let runtime = MediaFoundation::new()?;
        unsafe {
            let transform: IMFTransform =
                CoCreateInstance(&AACMFTEncoder, None, CLSCTX_INPROC_SERVER)?;
            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
            for (key, value) in [
                (MF_MT_AUDIO_NUM_CHANNELS, 2),
                (MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000),
                (MF_MT_AUDIO_BITS_PER_SAMPLE, 16),
                (MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000),
                (MF_MT_AAC_PAYLOAD_TYPE, 0),
            ] {
                output.SetUINT32(&key, value)?;
            }
            transform.SetOutputType(0, &output, 0)?;
            let input = MFCreateMediaType()?;
            input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
            for (key, value) in [
                (MF_MT_AUDIO_NUM_CHANNELS, 2),
                (MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000),
                (MF_MT_AUDIO_BITS_PER_SAMPLE, 16),
                (MF_MT_AUDIO_BLOCK_ALIGNMENT, 4),
                (MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000),
            ] {
                input.SetUINT32(&key, value)?;
            }
            transform.SetInputType(0, &input, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
            let output_size = transform.GetOutputStreamInfo(0)?.cbSize.max(16_384);
            Ok(Self {
                transform,
                output_size,
                output_index: 0,
                stream: Stream::Audio,
                _runtime: runtime,
            })
        }
    }
    pub(super) fn encode(
        &mut self,
        frames: &[[f32; 2]],
        pts: u64,
        budget: &Arc<Budget>,
        packets: &Sender<Packet>,
    ) -> Result<()> {
        unsafe {
            let input = MFCreateSample()?;
            let buffer = MFCreateMemoryBuffer((frames.len() * 4) as u32)?;
            let mut ptr = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None)?;
            let target = std::slice::from_raw_parts_mut(ptr as *mut i16, frames.len() * 2);
            for (i, frame) in frames.iter().enumerate() {
                for ch in 0..2 {
                    target[i * 2 + ch] = (frame[ch].clamp(-1.0, 1.0) * 32767.0) as i16;
                }
            }
            buffer.Unlock()?;
            buffer.SetCurrentLength((frames.len() * 4) as u32)?;
            input.AddBuffer(&buffer)?;
            input.SetSampleTime(pts as i64)?;
            input.SetSampleDuration((frames.len() as u64 * TICKS / 48_000) as i64)?;
            self.transform.ProcessInput(0, &input, 0)?;
            loop {
                let output = MFCreateSample()?;
                output.AddBuffer(&MFCreateMemoryBuffer(self.output_size)?)?;
                let mut out = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(Some(output)),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut flags = 0;
                let result = self.transform.ProcessOutput(0, &mut out, &mut flags);
                let sample = ManuallyDrop::take(&mut out[0].pSample);
                let _events = ManuallyDrop::take(&mut out[0].pEvents);
                match result {
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => break,
                    Err(e) => return Err(e.into()),
                    Ok(()) => {}
                }
                if let Some(sample) = sample {
                    let buffer = sample.ConvertToContiguousBuffer()?;
                    let mut ptr = std::ptr::null_mut();
                    let mut len = 0;
                    buffer.Lock(&mut ptr, None, Some(&mut len))?;
                    let bytes = std::slice::from_raw_parts(ptr, len as usize).to_vec();
                    buffer.Unlock()?;
                    // AAC output frames are consecutive 1024-sample blocks. This
                    // common timeline starts at zero, including startup silence.
                    let time = self.output_index * 1024 * TICKS / 48_000;
                    self.output_index += 1;
                    if let Some(data) = budget.allocate(bytes) {
                        let _ = packets.try_send(Packet {
                            stream: self.stream,
                            pts: time,
                            duration: 1024 * TICKS / 48_000,
                            keyframe: true,
                            data,
                        });
                    }
                }
            }
            Ok(())
        }
    }
}
