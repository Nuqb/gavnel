//! Stateful 48 kHz microphone processing shared by recording and mic tests.
use crate::config::{NoiseSuppression, Settings};
use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use std::sync::Arc;

const HOP: usize = 480;
const WINDOW: usize = HOP * 2;

pub struct MicrophoneProcessor {
    filters: Vec<Filter>,
    input: [[f32; 2]; HOP],
    output: [[f32; 2]; HOP],
    cursor: usize,
    envelope: f32,
    gain: f32,
    hold: usize,
    open: bool,
}
impl MicrophoneProcessor {
    pub fn new(settings: &Settings) -> Self {
        let channels = if settings.mic_mono { 1 } else { 2 };
        Self {
            filters: (0..channels)
                .filter_map(|_| match settings.noise_suppression_mode {
                    NoiseSuppression::None => None,
                    NoiseSuppression::Standard => Some(Filter::Standard(Box::new(Spectral::new()))),
                    NoiseSuppression::Neural => {
                        Some(Filter::Neural(nnnoiseless::DenoiseState::new()))
                    }
                })
                .collect(),
            input: [[0.0; 2]; HOP],
            output: [[0.0; 2]; HOP],
            cursor: 0,
            envelope: 0.0,
            gain: 0.0,
            hold: 0,
            open: false,
        }
    }

    /// Both filters have 10 ms analysis delay plus 10 ms framing delay.
    /// Read the mic this far ahead on the existing 80 ms mixer buffer to keep
    /// filtered voice aligned with video and PC audio. None is zero latency.
    pub fn lookahead(&self) -> i64 {
        if self.filters.is_empty() {
            0
        } else {
            WINDOW as i64
        }
    }

    pub fn process(&mut self, samples: &mut [[f32; 2]], settings: &Settings, held: bool) {
        let threshold = 10.0f32.powf(settings.input_sensitivity_db / 20.0);
        for sample in samples {
            let clean = sample.map(|v| {
                if v.is_finite() {
                    v.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            });
            if !self.filters.is_empty() {
                self.input[self.cursor] = clean;
                *sample = self.output[self.cursor];
                self.cursor += 1;
                if self.cursor == HOP {
                    let mut input = [0.0; HOP];
                    let mut output = [0.0; HOP];
                    for (channel, filter) in self.filters.iter_mut().enumerate() {
                        for (i, frame) in self.input.iter().enumerate() {
                            input[i] = frame[channel];
                        }
                        filter.process(&input, &mut output);
                        for (i, frame) in self.output.iter_mut().enumerate() {
                            frame[channel] = output[i];
                        }
                    }
                    if self.filters.len() == 1 {
                        for frame in &mut self.output {
                            frame[1] = frame[0];
                        }
                    }
                    self.cursor = 0;
                }
            } else {
                *sample = clean;
            }

            // Linked stereo RMS detection, 6 dB hysteresis, 150 ms hangover.
            // Smooth attack/release avoids clicks and chopped word endings.
            let power = sample[0].powi(2).max(sample[1].powi(2));
            let coefficient = if power > self.envelope { 0.02 } else { 0.002 };
            self.envelope += coefficient * (power - self.envelope);
            if self.envelope >= threshold.powi(2) {
                self.open = true;
                self.hold = 7200;
            } else if self.envelope < (threshold * 0.5).powi(2) {
                self.hold = self.hold.saturating_sub(1);
                if self.hold == 0 {
                    self.open = false;
                }
            }
            // PTT replaces voice activation: a held key must permit soft speech.
            if settings.push_to_talk && !held {
                *sample = [0.0; 2];
                self.gain = 0.0;
                continue;
            }
            let target =
                if !settings.input_sensitivity_enabled || settings.push_to_talk || self.open {
                    1.0
                } else {
                    0.0
                };
            if !settings.input_sensitivity_enabled || settings.push_to_talk {
                self.gain = 1.0;
            } else {
                let step = if target > self.gain {
                    1.0 / 96.0
                } else {
                    1.0 / 2880.0
                };
                self.gain += (target - self.gain).clamp(-step, step);
            }
            for value in sample {
                *value = (*value * self.gain * settings.mic_volume).clamp(-1.0, 1.0);
            }
        }
    }
}

enum Filter {
    Standard(Box<Spectral>),
    Neural(Box<nnnoiseless::DenoiseState<'static>>),
}
impl Filter {
    fn process(&mut self, input: &[f32; HOP], output: &mut [f32; HOP]) {
        match self {
            Self::Standard(filter) => filter.process(input, output),
            Self::Neural(filter) => {
                let scaled = input.map(|v| v * 32768.0);
                filter.process_frame(output, &scaled);
                for value in output {
                    *value = (*value / 32768.0).clamp(-1.0, 1.0);
                }
            }
        }
    }
}

/// Conservative Wiener filtering with a rolling minimum noise estimate.
/// Square-root Hann analysis/synthesis with 50% overlap preserves continuity.
struct Spectral {
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    spectrum: Vec<Complex32>,
    scratch: Vec<Complex32>,
    window: [f32; WINDOW],
    previous: [f32; HOP],
    overlap: [f32; HOP],
    smoothed: [f32; WINDOW],
    minimum: [f32; WINDOW],
    last_minimum: [f32; WINDOW],
    noise: [f32; WINDOW],
    gain: [f32; WINDOW],
    frames: usize,
    active_frames: usize,
}
impl Spectral {
    fn new() -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(WINDOW);
        let ifft = planner.plan_fft_inverse(WINDOW);
        let scratch = vec![
            Complex32::default();
            fft.get_inplace_scratch_len()
                .max(ifft.get_inplace_scratch_len())
        ];
        Self {
            fft,
            ifft,
            spectrum: vec![Complex32::default(); WINDOW],
            scratch,
            window: std::array::from_fn(|i| {
                (std::f32::consts::PI * (i as f32 + 0.5) / WINDOW as f32).sin()
            }),
            previous: [0.0; HOP],
            overlap: [0.0; HOP],
            smoothed: [0.0; WINDOW],
            minimum: [f32::MAX; WINDOW],
            // A zero here pins the noise estimate to silence for a full second.
            last_minimum: [f32::MAX; WINDOW],
            noise: [0.0; WINDOW],
            gain: [1.0; WINDOW],
            frames: 0,
            active_frames: 0,
        }
    }
    fn process(&mut self, input: &[f32; HOP], output: &mut [f32; HOP]) {
        for i in 0..WINDOW {
            let value = if i < HOP {
                self.previous[i]
            } else {
                input[i - HOP]
            };
            self.spectrum[i] = Complex32::new(value * self.window[i], 0.0);
        }
        self.previous.copy_from_slice(input);
        self.fft
            .process_with_scratch(&mut self.spectrum, &mut self.scratch);
        let audible = self.spectrum.iter().map(|bin| bin.norm_sqr()).sum::<f32>() > 1e-10;
        self.active_frames = if audible { self.active_frames + 1 } else { 0 };
        for i in 0..WINDOW {
            let power = self.spectrum[i].norm_sqr();
            self.smoothed[i] = 0.8 * self.smoothed[i] + 0.2 * power;
            // Do not train on startup silence or isolated transients. Neither
            // should poison the minimum statistics for the next two seconds.
            if self.active_frames >= 5 && self.smoothed[i] > 1e-16 {
                self.minimum[i] = self.minimum[i].min(self.smoothed[i]);
                let estimate = self.minimum[i].min(self.last_minimum[i]);
                self.noise[i] = 0.9 * self.noise[i] + 0.1 * estimate;
            }
            // Use the Wiener gain directly. Taking its square root previously
            // made the reduction barely audible on common broadband mic noise.
            let target = (1.0 - 2.5 * self.noise[i] / power.max(1e-16)).max(0.08);
            self.gain[i] = 0.6 * self.gain[i] + 0.4 * target;
            self.spectrum[i] *= self.gain[i];
        }
        self.frames += 1;
        if self.frames.is_multiple_of(100) {
            self.last_minimum = self.minimum;
            self.minimum.fill(f32::MAX);
        }
        self.ifft
            .process_with_scratch(&mut self.spectrum, &mut self.scratch);
        for (i, value) in output.iter_mut().enumerate() {
            *value = self.overlap[i] + self.spectrum[i].re * self.window[i] / WINDOW as f32;
            self.overlap[i] = self.spectrum[i + HOP].re * self.window[i + HOP] / WINDOW as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings(mode: NoiseSuppression) -> Settings {
        Settings {
            noise_suppression_mode: mode,
            ..Settings::default()
        }
    }
    fn energy(samples: &[[f32; 2]]) -> f32 {
        samples.iter().map(|v| v[0] * v[0]).sum::<f32>() / samples.len() as f32
    }
    #[test]
    fn bypass_preserves_stereo_and_volume_and_ptt_overrides_sensitivity() {
        let mut s = settings(NoiseSuppression::None);
        s.mic_volume = 0.5;
        let mut processor = MicrophoneProcessor::new(&s);
        let mut input = [[0.4, -0.2]; 1024];
        processor.process(&mut input, &s, true);
        assert_eq!(input, [[0.2, -0.1]; 1024]);
        assert_eq!(processor.lookahead(), 0);
        s.input_sensitivity_enabled = true;
        s.input_sensitivity_db = -25.0;
        s.push_to_talk = true;
        let mut input = [[0.001; 2]; 1024];
        processor.process(&mut input, &s, true);
        assert_eq!(input, [[0.0005; 2]; 1024]);
        processor.process(&mut input, &s, false);
        assert_eq!(input, [[0.0; 2]; 1024]);
    }
    #[test]
    fn sensitivity_has_hangover_and_smooth_release() {
        let s = Settings {
            input_sensitivity_enabled: true,
            input_sensitivity_db: -50.0,
            ..Settings::default()
        };
        let mut p = MicrophoneProcessor::new(&s);
        let mut quiet = [[0.0001; 2]; 1024];
        p.process(&mut quiet, &s, true);
        assert_eq!(energy(&quiet), 0.0);
        let mut voice = [[0.1; 2]; 1024];
        p.process(&mut voice, &s, true);
        assert!(voice[0][0] < voice[500][0]);
        assert!((voice[500][0] - 0.1).abs() < 1e-6);
        let mut ending = [[0.0001; 2]; 4800];
        p.process(&mut ending, &s, true);
        assert!(ending.iter().all(|v| v[0] > 0.0));
        let mut pause = vec![[0.0001; 2]; 24000];
        p.process(&mut pause, &s, true);
        assert_eq!(pause.last().unwrap(), &[0.0; 2]);
        assert!(
            pause
                .windows(2)
                .all(|v| (v[1][0] - v[0][0]).abs() < 0.00001)
        );
    }
    #[test]
    fn framing_is_independent_of_capture_packet_sizes() {
        for mode in [NoiseSuppression::Standard, NoiseSuppression::Neural] {
            let s = settings(mode);
            let input: Vec<_> = (0..5000)
                .map(|i| [(i as f32 * 0.1).sin() * 0.1, 0.0])
                .collect();
            let mut a = input.clone();
            let mut b = input.clone();
            let mut p = MicrophoneProcessor::new(&s);
            let mut q = MicrophoneProcessor::new(&s);
            p.process(&mut a, &s, true);
            for chunk in b.chunks_mut(137) {
                q.process(chunk, &s, true);
            }
            assert_eq!(a, b);
            assert_eq!(p.lookahead(), 960);
            assert!(
                a.iter()
                    .all(|v| v[1] == 0.0 && v[0].is_finite() && v[0].abs() <= 1.0)
            );
        }
    }
    #[test]
    fn standard_reduces_stationary_noise_and_preserves_new_voice_energy() {
        let s = settings(NoiseSuppression::Standard);
        let mut p = MicrophoneProcessor::new(&s);
        let mut seed = 7u32;
        let mut noise: Vec<_> = (0..144000)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let value = (seed as f32 / u32::MAX as f32 - 0.5) * 0.02;
                [value; 2]
            })
            .collect();
        let original = energy(&noise[96000..]);
        p.process(&mut noise, &s, true);
        let filtered = energy(&noise[96000..]);
        assert!(
            filtered < original * 0.25,
            "noise energy ratio {}",
            filtered / original
        );
        let mut voice: Vec<_> = (0..24000)
            .map(|i| {
                let t = i as f32 / 48000.0;
                [(std::f32::consts::TAU * 220.0 * t).sin() * 0.1; 2]
            })
            .collect();
        p.process(&mut voice, &s, true);
        assert!(energy(&voice[2000..12000]) > 0.003);
    }
    #[test]
    fn standard_is_audibly_active_in_a_short_test_even_after_startup_silence() {
        for silence in [0, 48000 * 3] {
            let s = settings(NoiseSuppression::Standard);
            let mut p = MicrophoneProcessor::new(&s);
            p.process(&mut vec![[0.0; 2]; silence], &s, true);
            let mut seed = 37u32;
            let mut noise: Vec<_> = (0..48000)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    [(seed as f32 / u32::MAX as f32 - 0.5) * 0.02; 2]
                })
                .collect();
            let before = energy(&noise[12000..]);
            p.process(&mut noise, &s, true);
            let ratio = energy(&noise[12000..]) / before;
            assert!(
                ratio < 0.25,
                "Standard reduction after {silence} silent samples: {} dB",
                10.0 * ratio.log10()
            );
        }
    }
    #[test]
    fn neural_filter_reduces_noise_with_correct_pcm_scale_and_preserves_mono() {
        let mut s = settings(NoiseSuppression::Neural);
        s.mic_mono = true;
        let mut p = MicrophoneProcessor::new(&s);
        let mut seed = 17u32;
        let mut fan = 0.0;
        let mut signal: Vec<_> = (0..72000)
            .map(|i| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                fan = 0.85 * fan + 0.15 * (seed as f32 / u32::MAX as f32 - 0.5) * 0.02;
                [fan + (std::f32::consts::TAU * 80.0 * i as f32 / 48000.0).sin() * 0.005; 2]
            })
            .collect();
        let input_energy = energy(&signal[48000..]);
        p.process(&mut signal, &s, true);
        // The model is conservative at low levels; require measurable reduction,
        // rather than treating every kind of noise as a guaranteed 3 dB cut.
        assert!(
            energy(&signal[48000..]) < input_energy * 0.75,
            "neural noise energy ratio {}",
            energy(&signal[48000..]) / input_energy
        );
        assert!(signal.iter().all(|v| v[0].is_finite() && v[0] == v[1]));
    }
    #[test]
    fn neural_stream_matches_reference_engine_at_the_correct_scale_and_delay() {
        let mut s = settings(NoiseSuppression::Neural);
        s.mic_mono = true;
        let input: Vec<_> = (0..HOP * 8)
            .map(|i| [(i as f32 * 0.031).sin() * 0.1; 2])
            .collect();
        let mut actual = input.clone();
        MicrophoneProcessor::new(&s).process(&mut actual, &s, true);
        let mut reference = nnnoiseless::DenoiseState::new();
        for (frame, chunk) in input.as_chunks::<HOP>().0.iter().take(7).enumerate() {
            let scaled: [f32; HOP] = std::array::from_fn(|i| chunk[i][0] * 32768.0);
            let mut out = [0.0; HOP];
            reference.process_frame(&mut out, &scaled);
            for i in 0..HOP {
                assert_eq!(
                    actual[(frame + 1) * HOP + i][0],
                    (out[i] / 32768.0).clamp(-1.0, 1.0)
                );
            }
        }
    }
    #[test]
    fn standard_overlap_add_has_exact_compensated_timing_and_unity_gain() {
        let s = settings(NoiseSuppression::Standard);
        let mut p = MicrophoneProcessor::new(&s);
        let mut signal = vec![[0.0; 2]; 4000];
        signal[1500] = [0.5, -0.25];
        p.process(&mut signal, &s, true);
        assert!((signal[1500 + 960][0] - 0.5).abs() < 1e-5);
        assert!((signal[1500 + 960][1] + 0.25).abs() < 1e-5);
        assert!(
            signal
                .iter()
                .enumerate()
                .all(|(i, v)| i == 2460 || v[0].abs() < 1e-5)
        );
    }
}
