//! Microphone test capture, playback, and push-to-talk controls.
use super::*;
pub fn ptt_key_name(key: u16) -> String {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyNameTextW, MAPVK_VK_TO_VSC, MapVirtualKeyW,
    };
    let scan = unsafe { MapVirtualKeyW(key as u32, MAPVK_VK_TO_VSC) };
    let mut name = [0u16; 128];
    let len = unsafe { GetKeyNameTextW((scan << 16) as i32, &mut name) };
    if len > 0 {
        String::from_utf16_lossy(&name[..len as usize])
    } else {
        format!("Key {key}")
    }
}

pub(super) fn ptt_held(settings: &Settings) -> bool {
    !settings.push_to_talk
        || unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
                settings.push_to_talk_key as i32,
            ) < 0
        }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicTestState {
    Recording,
    Ready,
    Playing,
}
#[derive(Clone, Copy)]
enum MicTestCommand {
    End,
    Play,
    Stop,
    Quit,
}

fn same_microphone_settings(a: &Settings, b: &Settings) -> bool {
    a.input_device == b.input_device
        && a.mic_volume == b.mic_volume
        && a.mic_mono == b.mic_mono
        && a.mic_channel == b.mic_channel
        && a.noise_suppression_mode == b.noise_suppression_mode
        && a.input_sensitivity_enabled == b.input_sensitivity_enabled
        && a.input_sensitivity_db == b.input_sensitivity_db
        && a.push_to_talk == b.push_to_talk
        && a.push_to_talk_key == b.push_to_talk_key
}

pub struct MicTest {
    settings: Settings,
    level: Arc<std::sync::atomic::AtomicU32>,
    state: Arc<std::sync::atomic::AtomicU8>,
    has_recording: Arc<AtomicBool>,
    error: Arc<std::sync::Mutex<Option<String>>>,
    commands: std::sync::mpsc::Sender<MicTestCommand>,
    worker: Option<JoinHandle<()>>,
}
impl MicTest {
    pub fn matches_settings(&self, settings: &Settings) -> bool {
        same_microphone_settings(&self.settings, settings)
    }
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
    pub fn state(&self) -> MicTestState {
        match self.state.load(Ordering::Acquire) {
            0 => MicTestState::Recording,
            2 => MicTestState::Playing,
            _ => MicTestState::Ready,
        }
    }
    pub fn has_recording(&self) -> bool {
        self.has_recording.load(Ordering::Acquire)
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok()?.clone()
    }
    pub fn end(&self) {
        let _ = self.commands.send(MicTestCommand::End);
    }
    pub fn playback(&self) {
        let _ = self.commands.send(MicTestCommand::Play);
    }
    pub fn stop_playback(&self) {
        let _ = self.commands.send(MicTestCommand::Stop);
    }
    pub fn start(settings: Settings) -> Self {
        let (commands, receiver) = std::sync::mpsc::channel();
        let mut test = Self {
            settings: settings.clone(),
            level: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            state: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            has_recording: Arc::new(AtomicBool::new(false)),
            error: Arc::new(std::sync::Mutex::new(None)),
            commands,
            worker: None,
        };
        let level = test.level.clone();
        let state = test.state.clone();
        let has_recording = test.has_recording.clone();
        let error = test.error.clone();
        test.worker = Some(std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                unsafe {
                    CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
                }
                let result = (|| -> Result<()> {
                    let mut mic = Endpoint::open(true, &settings.input_device)?;
                    let mut processor = crate::microphone::MicrophoneProcessor::new(&settings);
                    let clock = Clock::new()?;
                    let mut frames = Vec::new();
                    let mut position = 0u64;
                    let mut play = false;
                    // Bound test memory, then retain the sample for explicit playback.
                    while clock.now() < 60 * TICKS {
                        match receiver.try_recv() {
                            Ok(MicTestCommand::Quit)
                            | Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(()),
                            Ok(MicTestCommand::End) => break,
                            Ok(MicTestCommand::Play) => {
                                play = true;
                                break;
                            }
                            _ => {}
                        }
                        mic.read(&clock)?;
                        let ready = (clock.now() * 48_000 / TICKS).saturating_sub(3840);
                        while position + 1024 <= ready {
                            let mut block = [[0.0f32; 2]; 1024];
                            mic.mix(
                                position as i64 + processor.lookahead(),
                                &mut block,
                                1.0,
                                settings.mic_mono.then_some(settings.mic_channel),
                            );
                            let peak = block
                                .iter()
                                .flatten()
                                .map(|v| v.abs())
                                .fold(0.0f32, f32::max);
                            // Raw dBFS level helps choose the sensitivity threshold, even with the gate closed.
                            let db = 20.0 * peak.max(0.00001).log10();
                            level.store(
                                ((db + 100.0) / 100.0).clamp(0.0, 1.0).to_bits(),
                                Ordering::Relaxed,
                            );
                            processor.process(&mut block, &settings, ptt_held(&settings));
                            frames.extend_from_slice(&block);
                            position += 1024;
                        }
                        std::thread::sleep(Duration::from_millis(4));
                    }
                    drop(mic);
                    level.store(0, Ordering::Relaxed);
                    has_recording.store(!frames.is_empty(), Ordering::Release);
                    let wave = microphone_test_wave(&frames);
                    let duration = Duration::from_secs_f64(frames.len() as f64 / 48_000.0);
                    loop {
                        if !play {
                            state.store(1, Ordering::Release);
                            match receiver.recv() {
                                Ok(MicTestCommand::Play) => play = true,
                                Ok(MicTestCommand::Quit) | Err(_) => return Ok(()),
                                _ => continue,
                            }
                        }
                        if play && !frames.is_empty() {
                            state.store(2, Ordering::Release);
                            use windows::Win32::Media::Audio::{
                                PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT,
                            };
                            anyhow::ensure!(
                                unsafe {
                                    PlaySoundW(
                                        PCWSTR(wave.as_ptr().cast()),
                                        None,
                                        SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
                                    )
                                }
                                .as_bool(),
                                "Microphone playback failed"
                            );
                            let quit = wait_for_mic_playback(&receiver, duration, &frames, &level);
                            // Stop before releasing the in-memory wave, including on cancellation.
                            unsafe {
                                let _ = PlaySoundW(PCWSTR::null(), None, SND_ASYNC);
                            }
                            level.store(0, Ordering::Relaxed);
                            if quit {
                                return Ok(());
                            }
                        }
                        play = false;
                    }
                })();
                unsafe {
                    CoUninitialize();
                }
                result
            })();
            if let Err(e) = result {
                *error.lock().unwrap() = Some(format!("{e:#}"));
            }
            level.store(0, Ordering::Relaxed);
            state.store(1, Ordering::Release);
        }));
        test
    }
}
impl Drop for MicTest {
    fn drop(&mut self) {
        let _ = self.commands.send(MicTestCommand::Quit);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn microphone_test_wave(frames: &[[f32; 2]]) -> Vec<u8> {
    let bytes = (frames.len() * 4) as u32;
    let mut wave = Vec::with_capacity(44 + bytes as usize);
    wave.extend_from_slice(b"RIFF");
    wave.extend_from_slice(&(36 + bytes).to_le_bytes());
    wave.extend_from_slice(b"WAVEfmt ");
    wave.extend_from_slice(&16u32.to_le_bytes());
    wave.extend_from_slice(&1u16.to_le_bytes());
    wave.extend_from_slice(&2u16.to_le_bytes());
    wave.extend_from_slice(&48_000u32.to_le_bytes());
    wave.extend_from_slice(&192_000u32.to_le_bytes());
    wave.extend_from_slice(&4u16.to_le_bytes());
    wave.extend_from_slice(&16u16.to_le_bytes());
    wave.extend_from_slice(b"data");
    wave.extend_from_slice(&bytes.to_le_bytes());
    for sample in frames.iter().flatten() {
        wave.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    wave
}

fn wait_for_mic_playback(
    receiver: &std::sync::mpsc::Receiver<MicTestCommand>,
    duration: Duration,
    frames: &[[f32; 2]],
    level: &std::sync::atomic::AtomicU32,
) -> bool {
    let started = Instant::now();
    while started.elapsed() < duration {
        let remaining = duration.saturating_sub(started.elapsed());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(20))) {
            Ok(MicTestCommand::Quit) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return true;
            }
            Ok(MicTestCommand::Stop) | Ok(MicTestCommand::End) => return false,
            _ => {}
        }
        let frame = ((started.elapsed().as_secs_f64() * 48_000.0) as usize).min(frames.len());
        let peak = frames[frame..frames.len().min(frame + 1024)]
            .iter()
            .flatten()
            .map(|v| v.abs())
            .fold(0.0f32, f32::max);
        let db = 20.0 * peak.max(0.00001).log10();
        level.store(
            ((db + 100.0) / 100.0).clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
    }
    false
}

#[cfg(test)]
mod microphone_test_controls {
    use super::*;
    #[test]
    fn microphone_tests_expire_when_the_filter_changes_but_not_for_unrelated_settings() {
        let original = Settings::default();
        let mut changed = original.clone();
        changed.desktop_volume = 0.2;
        assert!(same_microphone_settings(&original, &changed));
        changed.noise_suppression_mode = crate::config::NoiseSuppression::Standard;
        assert!(!same_microphone_settings(&original, &changed));
        changed = original.clone();
        changed.input_sensitivity_db = -55.0;
        assert!(!same_microphone_settings(&original, &changed));
        changed = original.clone();
        changed.input_device = "Another microphone".into();
        assert!(!same_microphone_settings(&original, &changed));
    }
    #[test]
    fn playback_stops_promptly_and_keeps_sample_for_replay() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let frames = vec![[0.25, -0.25]; 1024];
        let original = frames.clone();
        let level = std::sync::atomic::AtomicU32::new(0);
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            sender.send(MicTestCommand::Stop).unwrap();
        });
        let started = Instant::now();
        assert!(!wait_for_mic_playback(
            &receiver,
            Duration::from_secs(60),
            &frames,
            &level
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(frames, original);
        worker.join().unwrap();
    }
    #[test]
    fn dropping_test_cancels_playback_wait() {
        let (sender, receiver) = std::sync::mpsc::channel();
        drop(sender);
        assert!(wait_for_mic_playback(
            &receiver,
            Duration::from_secs(60),
            &[],
            &std::sync::atomic::AtomicU32::new(0)
        ));
    }
    #[test]
    fn recorded_sample_is_reusable_pcm_wave() {
        let wave = microphone_test_wave(&[[1.0, -1.0]; 480]);
        assert_eq!(&wave[..4], b"RIFF");
        assert_eq!(wave.len(), 44 + 480 * 4);
        assert_eq!(u32::from_le_bytes(wave[40..44].try_into().unwrap()), 1920);
        assert_eq!(&wave[44..46], &32767i16.to_le_bytes());
        assert_eq!(&wave[46..48], &(-32767i16).to_le_bytes());
    }
}
