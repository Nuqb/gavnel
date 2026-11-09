//! Explicit recording diagnostic; never runs during normal app startup.
use crate::{config, engine, platform};
pub(super) fn run(args: &[String]) -> anyhow::Result<()> {
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
    };
    let seconds = value("--seconds")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(8);
    let settings = config::Settings {
        recording_mode: if args.iter().any(|a| a == "--full-recording") {
            config::RecordingMode::FullRecording
        } else {
            config::RecordingMode::Clips
        },
        replay_seconds: value("--replay-seconds")
            .and_then(|s| s.parse().ok())
            .unwrap_or(60),
        mode: if args.iter().any(|a| a == "--game") {
            config::CaptureMode::Automatic
        } else {
            config::CaptureMode::Monitor
        },
        desktop_audio: !args.iter().any(|a| a == "--no-audio"),
        clip_feedback: config::ClipFeedback {
            sound: false,
            notification: false,
            ..Default::default()
        },
        clips_dir: value("--output")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("test-clips")),
        ..config::Settings::default()
    };
    let engine = engine::Engine::with_backend(
        engine::Notify {
            ctx: eframe::egui::Context::default(),
            visible: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
        args.iter().any(|a| a == "--dxgi"),
    );
    engine.send(engine::Command::Start(Box::new(settings)))?;
    let switch_folder = value("--switch-output").map(std::path::PathBuf::from);
    for elapsed in 0..seconds {
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("{:?}", engine.snapshot());
        if elapsed == seconds / 2 && args.iter().any(|a| a == "--also-clip") {
            engine.send(engine::Command::Save)?;
        }
        if elapsed == seconds / 2
            && let Some(folder) = &switch_folder
        {
            platform::validate_clip_folder(folder)?;
            let before = engine.snapshot().buffered;
            engine.send(engine::Command::SetOutputFolder(folder.clone()))?;
            std::thread::sleep(std::time::Duration::from_millis(100));
            let after = engine.snapshot().buffered;
            if after + 0.1 < before {
                anyhow::bail!("Changing the folder reset the replay buffer");
            }
            println!("Folder changed without reset: {before:.2}s -> {after:.2}s");
        }
    }
    let state = engine.snapshot();
    if !state.recording {
        anyhow::bail!("Capture test failed: {}", state.message);
    }
    let saved_before = state.saved_count;
    if state.full_recording {
        engine.send(engine::Command::Stop)?;
    } else {
        engine.send(engine::Command::Save)?;
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        let state = engine.snapshot();
        if state.saved_count > saved_before
            && let Some(path) = state.last_clip
        {
            if let Some(folder) = &switch_folder
                && !args.iter().any(|a| a == "--full-recording")
                && !path.starts_with(folder)
            {
                anyhow::bail!("Clip was saved to the wrong folder");
            }
            println!("Saved: {}", path.display());
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    anyhow::bail!("Clip save failed: {}", engine.snapshot().message);
}
