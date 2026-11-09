//! WASAPI endpoints, device timelines, and PCM mixing.
use super::*;
pub(super) struct Segment {
    pub(super) start: i64,
    pub(super) frames: Vec<[f32; 2]>,
}

pub(super) fn packet_timestamp(
    reported: i64,
    arrived: i64,
    frames: u32,
    timestamp_error: bool,
) -> i64 {
    // Playback drivers can report QPC in another epoch, or hundreds of ms
    // ahead of real time without setting TIMESTAMP_ERROR. Anchor those streams
    // to arrival once; device positions keep subsequent PCM continuous.
    if timestamp_error
        || reported > arrived.saturating_add(480)
        || reported < arrived.saturating_sub(48_000)
    {
        arrived.saturating_sub(frames as i64)
    } else {
        reported
    }
}

pub(super) fn trim_segments(segments: &mut VecDeque<Segment>) {
    // Bound PCM by duration: 32 small packets can cover less than the mixer's
    // 80ms waiting period, discarding valid audio before it can be mixed.
    let mut retained: usize = segments.iter().map(|segment| segment.frames.len()).sum();
    while retained > 96_000 || segments.len() > 4096 {
        if let Some(segment) = segments.pop_front() {
            retained -= segment.frames.len();
        } else {
            break;
        }
    }
}
pub(super) struct Endpoint {
    pub(super) client: IAudioClient,
    pub(super) capture: IAudioCaptureClient,
    pub(super) segments: VecDeque<Segment>,
    pub(super) timeline: CaptureTimeline,
    pub(super) trace: Option<std::io::BufWriter<std::fs::File>>,
}

pub(super) struct CaptureTimeline {
    pub(super) device_rate: u32,
    pub(super) anchor: Option<(u64, i64)>,
    pub(super) last_device: u64,
    pub(super) next_frame: i64,
}
impl Default for CaptureTimeline {
    fn default() -> Self {
        Self::new(48_000)
    }
}
impl CaptureTimeline {
    pub(super) fn new(device_rate: u32) -> Self {
        Self {
            device_rate: device_rate.max(1),
            anchor: None,
            last_device: 0,
            next_frame: 0,
        }
    }
    pub(super) fn place(
        &mut self,
        device_position: u64,
        frames: u32,
        qpc_frame: i64,
        discontinuity: bool,
    ) -> i64 {
        // GetBuffer's device position identifies consecutive source frames. Re-rounding
        // every QPC timestamp can overlap packets or leave single-sample holes, creating
        // periodic clicks even when the original stream is continuous.
        let start = match self.anchor {
            Some((origin, frame)) if !discontinuity && device_position >= self.last_device => {
                // Device positions remain in the native rate even when WASAPI
                // converts buffer samples to our requested 48 kHz format.
                let delta = (device_position - origin) as u128 * 48_000 / self.device_rate as u128;
                frame
                    .saturating_add(i64::try_from(delta).unwrap_or(i64::MAX))
                    .max(self.next_frame)
            }
            anchor => {
                let start = if anchor.is_some() {
                    qpc_frame.max(self.next_frame)
                } else {
                    qpc_frame
                };
                self.anchor = Some((device_position, start));
                start
            }
        };
        self.last_device = device_position;
        self.next_frame = start.saturating_add(frames as i64);
        start
    }
}
impl Endpoint {
    pub(super) fn open(input: bool, id: &str) -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let device = if id.is_empty() {
                enumerator
                    .GetDefaultAudioEndpoint(if input { eCapture } else { eRender }, eConsole)?
            } else {
                let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
                enumerator.GetDevice(PCWSTR(wide.as_ptr()))?
            };
            let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
            let native_format = client.GetMixFormat()?;
            let native_rate = (*native_format).nSamplesPerSec;
            CoTaskMemFree(Some(native_format.cast()));
            anyhow::ensure!(
                native_rate > 0,
                "Audio device reported an invalid sample rate"
            );
            let format = WAVEFORMATEX {
                wFormatTag: 3,
                nChannels: 2,
                nSamplesPerSec: 48_000,
                nAvgBytesPerSec: 48_000 * 8,
                nBlockAlign: 8,
                wBitsPerSample: 32,
                cbSize: 0,
            };
            let flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY
                | if input {
                    0
                } else {
                    AUDCLNT_STREAMFLAGS_LOOPBACK
                };
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 1_000_000, 0, &format, None)?;
            let capture: IAudioCaptureClient = client.GetService()?;
            client.Start()?;
            Ok(Self {
                client,
                capture,
                segments: VecDeque::new(),
                timeline: CaptureTimeline::new(native_rate),
                trace: None,
            })
        }
    }
    pub(super) fn read(&mut self, clock: &Clock) -> Result<()> {
        unsafe {
            for _ in 0..32 {
                if self.capture.GetNextPacketSize()? == 0 {
                    break;
                }
                let mut ptr = std::ptr::null_mut();
                let mut frames = 0;
                let mut flags = 0;
                let mut qpc = 0;
                let mut device_position = 0;
                self.capture.GetBuffer(
                    &mut ptr,
                    &mut frames,
                    &mut flags,
                    Some(&mut device_position),
                    Some(&mut qpc),
                )?;
                if frames == 0 {
                    break;
                }
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                let data = if silent || ptr.is_null() {
                    vec![[0.0; 2]; frames as usize]
                } else {
                    std::slice::from_raw_parts(ptr as *const [f32; 2], frames as usize).to_vec()
                };
                self.capture.ReleaseBuffer(frames)?;
                let relative = if flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0 {
                    clock.now() as i128 - frames as i128 * TICKS as i128 / 48_000
                } else {
                    qpc as i128 - clock.origin as i128
                };
                let reported_frame = (relative * 48_000 / TICKS as i128) as i64;
                let arrived = (clock.now() * 48_000 / TICKS) as i64;
                let qpc_frame = packet_timestamp(
                    reported_frame,
                    arrived,
                    frames,
                    flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0,
                );
                let start = self.timeline.place(
                    device_position,
                    frames,
                    qpc_frame,
                    flags
                        & (AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32
                            | AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32)
                        != 0,
                );
                if let Some(trace) = &mut self.trace {
                    use std::io::Write;
                    writeln!(
                        trace,
                        "{device_position},{frames},{reported_frame},{start},{flags},{arrived}"
                    )?;
                }
                self.segments.push_back(Segment {
                    start,
                    frames: data,
                });
            }
            trim_segments(&mut self.segments);
            Ok(())
        }
    }
    pub(super) fn mix(
        &mut self,
        start: i64,
        output: &mut [[f32; 2]],
        gain: f32,
        mono: Option<crate::config::MicChannel>,
    ) {
        mix_segments(&mut self.segments, start, output, gain, mono);
    }
}
pub(super) fn mix_segments(
    segments: &mut VecDeque<Segment>,
    start: i64,
    output: &mut [[f32; 2]],
    gain: f32,
    mono: Option<crate::config::MicChannel>,
) {
    let end = start + output.len() as i64;
    for seg in segments.iter() {
        let seg_end = seg.start + seg.frames.len() as i64;
        let first = start.max(seg.start);
        let last = end.min(seg_end);
        for frame in first..last {
            let src = seg.frames[(frame - seg.start) as usize];
            let src = mono.map_or(src, |channel| channel.mono(src));
            let dst = &mut output[(frame - start) as usize];
            dst[0] += src[0] * gain;
            dst[1] += src[1] * gain;
        }
    }
    while segments
        .front()
        .is_some_and(|s| s.start + s.frames.len() as i64 <= end)
    {
        segments.pop_front();
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}
