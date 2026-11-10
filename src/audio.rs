mod aac;
mod capture;
mod diagnostics;
mod mic_test;
pub mod process;
mod worker;
use crate::{
    config::Settings,
    replay::{Budget, Packet, Stream, TICKS},
};
use aac::AacEncoder;
use anyhow::{Context, Result};
use capture::*;
use crossbeam_channel::Sender;
pub use diagnostics::{diagnostic, diagnostic_pc, format_report, reencode_diagnostic};
use mic_test::ptt_held;
pub use mic_test::{MicTest, MicTestState, ptt_key_name};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    mem::ManuallyDrop,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Media::{Audio::*, MediaFoundation::*},
        System::{
            Com::*,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
    },
    core::PCWSTR,
};

#[derive(Clone)]
pub struct Clock {
    pub origin: u64,
    frequency: i64,
}
impl Clock {
    pub fn new() -> Result<Self> {
        unsafe {
            let mut frequency = 0;
            QueryPerformanceFrequency(&mut frequency)?;
            let mut counter = 0;
            QueryPerformanceCounter(&mut counter)?;
            Ok(Self {
                origin: (counter as u128 * TICKS as u128 / frequency as u128) as u64,
                frequency,
            })
        }
    }
    pub fn now(&self) -> u64 {
        unsafe {
            let mut c = 0;
            let _ = QueryPerformanceCounter(&mut c);
            ((c as u128 * TICKS as u128 / self.frequency as u128) as u64)
                .saturating_sub(self.origin)
        }
    }
}

#[derive(Clone, Debug)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
}
fn enumerate_devices(input: bool) -> Result<Vec<AudioDevice>> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let list = enumerator
            .EnumAudioEndpoints(if input { eCapture } else { eRender }, DEVICE_STATE_ACTIVE)?;
        let mut result = Vec::new();
        for index in 0..list.GetCount()? {
            let device = list.Item(index)?;
            let ptr = device.GetId()?;
            let id = ptr.to_string()?;
            CoTaskMemFree(Some(ptr.0 as _));
            let name = (|| -> windows::core::Result<String> {
                use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
                let props = device.OpenPropertyStore(STGM_READ)?;
                let value = props.GetValue(&PKEY_Device_FriendlyName)?;
                // PROPVARIANT owns the allocated string and clears it on drop.
                let raw = &value.Anonymous.Anonymous;
                Ok(raw.Anonymous.pwszVal.to_string()?)
            })()
            .unwrap_or_else(|_| format!("Audio device {}", index + 1));
            result.push(AudioDevice { id, label: name });
        }
        Ok(result)
    }
}
pub fn devices(input: bool) -> Vec<AudioDevice> {
    enumerate_devices(input).unwrap_or_default()
}

// Keep working devices alive while adding new outputs and retrying failed ones.
// A single default endpoint misses apps routed to chat/headset/virtual outputs.
fn refresh_outputs<T>(
    endpoints: &mut HashMap<String, T>,
    devices: &[AudioDevice],
    mut open: impl FnMut(&str) -> Result<T>,
) -> Vec<(String, String)> {
    endpoints.retain(|id, _| devices.iter().any(|d| &d.id == id));
    let mut errors = Vec::new();
    for device in devices {
        if let std::collections::hash_map::Entry::Vacant(entry) = endpoints.entry(device.id.clone())
        {
            match open(&device.id) {
                Ok(endpoint) => {
                    entry.insert(endpoint);
                }
                Err(error) => errors.push((
                    device.id.clone(),
                    format!(
                        "PC audio on {} unavailable; retrying: {error:#}",
                        device.label
                    ),
                )),
            }
        }
    }
    errors
}

pub struct AudioWorker {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl AudioWorker {
    pub fn start(
        settings: Settings,
        clock: Clock,
        output: crate::replay::EncoderOutput,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let warnings = output.messages.clone();
        let worker = std::thread::Builder::new()
            .name("gavnel-audio".into())
            .spawn(move || {
                if let Err(error) = worker::run(settings, clock, output, signal) {
                    let _ = warnings.try_send(format!("Audio recording stopped: {error:#}"));
                }
            })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for AudioWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

pub fn track_label(stream: Stream, names: &[String]) -> String {
    match stream {
        Stream::Desktop => "PC Audio".into(),
        Stream::Microphone => "Microphone".into(),
        Stream::Application(i) => names
            .get(i as usize)
            .cloned()
            .unwrap_or_else(|| format!("Application {}", i + 1)),
        _ => "Combined".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_driver_timestamps_do_not_discard_pc_audio_before_mixing() {
        // Reproduce both observed drivers: a ~0.8s future timestamp and a
        // virtual device reporting QPC in a completely different epoch.
        for offset in [37_791, -35_417_756_200i64] {
            let mut timeline = CaptureTimeline::default();
            let mut old_segments = VecDeque::new();
            let mut fixed_segments = VecDeque::new();
            let mut old_position = 0;
            let mut position = 0;
            let mut audible = 0;
            let mut previously_audible = 0;
            let mut old_timeline = CaptureTimeline::default();
            for packet in 0..1000u64 {
                let arrived = packet as i64 * 480;
                let reported = arrived + offset;
                let data = vec![[0.2, 0.1]; 480];
                old_segments.push_back(Segment {
                    start: old_timeline.place(packet * 480, 480, reported, false),
                    frames: data.clone(),
                });
                while old_segments.len() > 32 {
                    old_segments.pop_front();
                }
                fixed_segments.push_back(Segment {
                    start: timeline.place(
                        packet * 480,
                        480,
                        packet_timestamp(reported, arrived, 480, false),
                        false,
                    ),
                    frames: data,
                });
                trim_segments(&mut fixed_segments);
                let ready = arrived.saturating_sub(3840);
                while position + 1024 <= ready {
                    let mut block = [[0.0; 2]; 1024];
                    mix_segments(&mut fixed_segments, position, &mut block, 1.0, None);
                    audible += block.iter().filter(|frame| frame[0] > 0.1).count();
                    position += 1024;
                }
                while old_position + 1024 <= ready {
                    let mut block = [[0.0; 2]; 1024];
                    mix_segments(&mut old_segments, old_position, &mut block, 1.0, None);
                    previously_audible += block.iter().filter(|frame| frame[0] > 0.1).count();
                    old_position += 1024;
                }
            }
            assert_eq!(previously_audible, 0, "Did not reproduce missing PC audio");
            assert!(
                audible > 470_000,
                "Corrected PC audio is missing: {audible}"
            );
        }
        assert_eq!(packet_timestamp(500, 700, 480, false), 500);
        assert_eq!(packet_timestamp(500, 700, 480, true), 220);
    }
    #[test]
    fn small_driver_packets_survive_mixer_latency_with_bounded_pcm_memory() {
        let mut segments = VecDeque::new();
        for packet in 0..100 {
            segments.push_back(Segment {
                start: packet * 48,
                frames: vec![[0.1; 2]; 48],
            });
        }
        trim_segments(&mut segments);
        assert_eq!(
            segments.len(),
            100,
            "100ms of PCM must survive the 80ms mixing delay"
        );
        for packet in 100..4000 {
            segments.push_back(Segment {
                start: packet * 48,
                frames: vec![[0.1; 2]; 48],
            });
        }
        trim_segments(&mut segments);
        assert_eq!(
            segments.iter().map(|s| s.frames.len()).sum::<usize>(),
            96_000
        );
    }
    #[test]
    fn automatic_outputs_keep_working_streams_deduplicate_retry_and_follow_changes() {
        let device = |id: &str| AudioDevice {
            id: id.into(),
            label: id.into(),
        };
        let mut streams = HashMap::new();
        let mut opened = Vec::new();
        let errors = refresh_outputs(
            &mut streams,
            &[
                device("game"),
                device("chat"),
                device("game"),
                device("unplugged"),
            ],
            |id| {
                opened.push(id.to_owned());
                anyhow::ensure!(id != "unplugged", "device unavailable");
                Ok(id.to_owned())
            },
        );
        assert_eq!(opened, ["game", "chat", "unplugged"]);
        assert_eq!(streams.len(), 2);
        assert_eq!(errors.len(), 1);
        opened.clear();
        let errors = refresh_outputs(
            &mut streams,
            &[device("chat"), device("unplugged"), device("headphones")],
            |id| {
                opened.push(id.to_owned());
                Ok(format!("new-{id}"))
            },
        );
        assert!(errors.is_empty());
        assert_eq!(opened, ["unplugged", "headphones"]);
        assert!(!streams.contains_key("game"));
        assert_eq!(streams["chat"], "chat");
        assert_eq!(streams["unplugged"], "new-unplugged");
    }
    #[test]
    fn game_chat_and_optional_microphone_mix_on_independent_device_timelines() {
        let mut outputs = Vec::new();
        for (rate, stereo) in [(48_000, [0.2, 0.1]), (96_000, [0.1, 0.2])] {
            let mut timeline = CaptureTimeline::new(rate);
            let mut segments = VecDeque::new();
            for packet in 0..100u64 {
                segments.push_back(Segment {
                    start: timeline.place(
                        packet * rate as u64 / 100,
                        480,
                        packet as i64 * 480,
                        false,
                    ),
                    frames: vec![stereo; 480],
                });
            }
            outputs.push(segments);
        }
        let mut pc_only = vec![[0.0; 2]; 48_000];
        for source in &mut outputs {
            mix_segments(source, 0, &mut pc_only, 1.0, None);
        }
        assert!(
            pc_only
                .iter()
                .all(|f| (f[0] - 0.3f32).abs() < 0.00001 && (f[1] - 0.3f32).abs() < 0.00001)
        );
        let mut microphone = VecDeque::from([Segment {
            start: 0,
            frames: vec![[0.1, 0.0]; 48_000],
        }]);
        let mut with_mic = pc_only.clone();
        mix_segments(
            &mut microphone,
            0,
            &mut with_mic,
            1.0,
            Some(crate::config::MicChannel::Left),
        );
        assert!(
            with_mic
                .iter()
                .all(|f| (f[0] - 0.4).abs() < 0.00001 && (f[1] - 0.4).abs() < 0.00001)
        );
        assert_eq!(pc_only[0], pc_only[47_999]);
    }
    #[test]
    fn device_frame_timing_eliminates_noise_from_packet_timestamp_rounding() {
        let reference: Vec<[f32; 2]> = (0..48_000)
            .map(|i| {
                let value = (std::f32::consts::TAU * 1000.0 * i as f32 / 48_000.0).sin() * 0.25;
                [value, value]
            })
            .collect();
        let mut timeline = CaptureTimeline::default();
        let mut rounded = VecDeque::new();
        let mut continuous = VecDeque::new();
        for packet in 0..100 {
            let position = packet * 480;
            let jitter = match packet % 3 {
                1 => 1,
                2 => -1,
                _ => 0,
            };
            let qpc = position as i64 + jitter;
            let frames = reference[position..position + 480].to_vec();
            rounded.push_back(Segment {
                start: qpc,
                frames: frames.clone(),
            });
            continuous.push_back(Segment {
                start: timeline.place(position as u64, 480, qpc, false),
                frames,
            });
        }
        let mut old_audio = vec![[0.0; 2]; reference.len()];
        let mut fixed_audio = old_audio.clone();
        mix_segments(&mut rounded, 0, &mut old_audio, 1.0, None);
        mix_segments(&mut continuous, 0, &mut fixed_audio, 1.0, None);
        let error = |audio: &[[f32; 2]]| {
            audio
                .iter()
                .zip(&reference)
                .map(|(a, b)| (a[0] - b[0]).powi(2) as f64)
                .sum::<f64>()
                / audio.len() as f64
        };
        assert!(
            error(&old_audio) > 0.000001,
            "Test did not reproduce timestamp crackle"
        );
        assert_eq!(error(&fixed_audio), 0.0);
    }
    #[test]
    fn device_timeline_keeps_real_gaps_and_does_not_overlap_after_a_reset() {
        let mut timeline = CaptureTimeline::default();
        assert_eq!(timeline.place(0, 480, 100, false), 100);
        assert_eq!(timeline.place(480, 480, 579, false), 580);
        assert_eq!(timeline.place(1440, 480, 1540, false), 1540);
        // A driver discontinuity/reset must not add the same time range twice.
        assert_eq!(timeline.place(0, 480, 2000, true), 2020);
        assert_eq!(timeline.place(480, 480, 2502, false), 2500);
    }
    #[test]
    fn resampled_device_counters_stay_continuous_at_44100_and_96000_hz() {
        for rate in [44_100u32, 96_000] {
            let mut timeline = CaptureTimeline::new(rate);
            for packet in 0..10_000u64 {
                let native_position = 100_000 + packet * rate as u64 / 100;
                let qpc_jitter = if packet == 0 {
                    0
                } else {
                    (packet as i64 % 9) - 4
                };
                let frame = timeline.place(
                    native_position,
                    480,
                    1000 + packet as i64 * 480 + qpc_jitter,
                    false,
                );
                assert_eq!(
                    frame,
                    1000 + packet as i64 * 480,
                    "rate {rate}, packet {packet}"
                );
            }
        }
    }
    #[test]
    fn aac_round_trip_preserves_silence_and_stereo_tones_without_capture_or_playback() {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        let result = (|| -> Result<()> {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("synthetic-audio.mp4");
            let (tx, rx) = crossbeam_channel::bounded(256);
            let budget = Budget::new(1024 * 1024);
            let mut encoder = AacEncoder::new()?;
            // Separate-track encoders can stop while the mixed encoder stays active.
            let extra = [AacEncoder::new()?, AacEncoder::new()?];
            drop(extra);
            for block in 0..144u64 {
                let mut frames = [[0.0; 2]; 1024];
                for (i, frame) in frames.iter_mut().enumerate() {
                    let sample = block * 1024 + i as u64;
                    if sample >= 48_000 {
                        let tone = (std::f64::consts::TAU * 1000.0 * sample as f64 / 48_000.0).sin()
                            as f32;
                        *frame = [tone * 0.2, tone * 0.1];
                    }
                }
                encoder.encode(&frames, block * 1024 * TICKS / 48_000, &budget, &tx)?;
            }
            drop(encoder);
            let mut writer = mp4::Mp4Writer::write_start(
                std::io::BufWriter::new(std::fs::File::create(&path)?),
                &mp4::Mp4Config {
                    major_brand: "isom".parse()?,
                    minor_version: 0,
                    compatible_brands: vec!["isom".parse()?],
                    timescale: 720_000,
                },
            )?;
            let mut track: mp4::TrackConfig = mp4::AacConfig {
                bitrate: 192_000,
                ..Default::default()
            }
            .into();
            track.timescale = 48_000;
            writer.add_track(&track)?;
            for packet in rx.try_iter() {
                writer.write_sample(
                    1,
                    &mp4::Mp4Sample {
                        start_time: packet.pts * 48_000 / TICKS,
                        duration: 1024,
                        rendering_offset: 0,
                        is_sync: true,
                        bytes: bytes::Bytes::copy_from_slice(&packet.data.bytes),
                    },
                )?;
            }
            writer.write_end()?;
            drop(writer);
            let decoded = crate::audio_check::decoded_frames(&path, 3)?;
            assert!(decoded.len() >= 96_000);
            let silence_peak = decoded[4096..32_000]
                .iter()
                .flatten()
                .copied()
                .map(f32::abs)
                .fold(0.0, f32::max);
            assert!(
                silence_peak < 0.0001,
                "AAC added noise to silence: {silence_peak}"
            );
            let tone = &decoded[72_000..96_000];
            for (ch, expected) in [(0, 0.2f64 / 2.0f64.sqrt()), (1, 0.1f64 / 2.0f64.sqrt())] {
                let rms = (tone.iter().map(|f| (f[ch] as f64).powi(2)).sum::<f64>()
                    / tone.len() as f64)
                    .sqrt();
                assert!(
                    (rms - expected).abs() < 0.008,
                    "Tone RMS channel {ch}: {rms}"
                );
            }
            Ok(())
        })();
        unsafe {
            CoUninitialize();
        }
        result.unwrap();
    }
    #[test]
    fn mixer_aligns_timestamped_packets_and_discards_old_data() {
        // Mixing is independent of device APIs and uses frame positions on
        // the common 48 kHz timeline.
        let segment = Segment {
            start: 100,
            frames: vec![[0.5, 0.25]; 4],
        };
        let mut out = [[0.0; 2]; 4];
        let mut segments = VecDeque::from([segment]);
        mix_segments(&mut segments, 102, &mut out, 1.0, None);
        assert_eq!(out, [[0.5, 0.25], [0.5, 0.25], [0.0, 0.0], [0.0, 0.0]]);
        assert!(segments.is_empty());
    }
    #[test]
    fn mono_is_applied_only_to_microphone_before_combining_sources() {
        let mut desktop = VecDeque::from([Segment {
            start: 10,
            frames: vec![[0.2, -0.1]; 2],
        }]);
        let mut mic = VecDeque::from([Segment {
            start: 11,
            frames: vec![[0.6, 0.0]; 2],
        }]);
        let mut out = [[0.0; 2]; 3];
        mix_segments(&mut desktop, 10, &mut out, 1.0, None);
        mix_segments(
            &mut mic,
            10,
            &mut out,
            0.5,
            Some(crate::config::MicChannel::Left),
        );
        assert_eq!(out[0], [0.2, -0.1]);
        assert!((out[1][0] - 0.5).abs() < 0.00001);
        assert!((out[1][1] - 0.2).abs() < 0.00001);
        assert_eq!(out[2], [0.3, 0.3]);
        assert!(desktop.is_empty() && mic.is_empty());
    }
}
