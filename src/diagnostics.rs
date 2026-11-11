//! Explicit command-line diagnostics, separate from desktop startup.
mod self_test;
use crate::{audio, audio_check, branding, config, motion_test, sources, verify, video};
pub fn early(args: &[String]) -> anyhow::Result<bool> {
    if args.iter().any(|arg| arg == "--editor-self-test") {
        crate::media::self_test()?;
        println!("Native editor inspection, previews, audio mixing, and export passed.");
        return Ok(true);
    }
    if let Some(index) = args.iter().position(|a| a == "--inspect-video") {
        let path = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("Provide a video path"))?;
        let info = crate::media::probe(std::path::Path::new(path))?;
        println!(
            "{}x{}, {:.3} seconds",
            info.width, info.height, info.duration
        );
        return Ok(true);
    }
    if args.iter().any(|a| a == "--settings-info") {
        let (settings, warning) = config::Settings::load_preserving_storage();
        println!("Gavnel {}", env!("CARGO_PKG_VERSION"));
        println!("Settings: {}", config::Settings::path().display());
        println!("Storage: {}", settings.clips_dir.display());
        println!("Recording mode: {:?}", settings.recording_mode);
        if !warning.is_empty() {
            println!("{warning}");
        }
        return Ok(true);
    }
    if args.iter().any(|a| a == "--motion-window") {
        return motion_test::run().map(|_| true);
    }
    if let Some(index) = args.iter().position(|a| a == "--export-icons") {
        return branding::export_windows_icon(std::path::Path::new(
            args.get(index + 1)
                .ok_or_else(|| anyhow::anyhow!("Provide an icon output folder"))?,
        ))
        .map(|_| true);
    }
    Ok(false)
}
pub fn run(args: &[String]) -> anyhow::Result<bool> {
    if args.iter().any(|a| a == "--probe") {
        println!("NVIDIA hardware encoder: {}", video::driver_available());
        for monitor in sources::monitors() {
            println!("{}", monitor.label);
        }
        println!("Visible capture windows: {}", sources::windows().len());
        for device in audio::devices(false) {
            println!("Output: {}", device.label);
        }
        for device in audio::devices(true) {
            println!("Input: {}", device.label);
        }
        return Ok(true);
    }
    if args.iter().any(|a| a == "--audio-formats") {
        return audio::format_report().map(|_| true);
    }
    if let Some(index) = args.iter().position(|a| a == "--pc-audio-diagnostic") {
        let folder = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("Provide a diagnostic output folder"))?;
        return audio::diagnostic_pc(std::path::Path::new(folder), 10).map(|_| true);
    }
    if let Some(index) = args.iter().position(|a| a == "--audio-diagnostic") {
        let folder = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("Provide a diagnostic output folder"))?;
        return audio::diagnostic(std::path::Path::new(folder), 15).map(|_| true);
    }
    if let Some(index) = args.iter().position(|a| a == "--audio-reencode") {
        let source = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("Provide a source waveform"))?;
        let output = args
            .get(index + 2)
            .ok_or_else(|| anyhow::anyhow!("Provide an output audio file"))?;
        return audio::reencode_diagnostic(
            std::path::Path::new(source),
            std::path::Path::new(output),
        )
        .map(|_| true);
    }
    if let Some(index) = args.iter().position(|a| a == "--audio-check") {
        let seconds = args
            .iter()
            .position(|a| a == "--seconds")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(15);
        return audio_check::inspect(
            std::path::Path::new(
                args.get(index + 1)
                    .ok_or_else(|| anyhow::anyhow!("Provide a recording path"))?,
            ),
            seconds,
        )
        .map(|_| true);
    }
    if let Some(index) = args.iter().position(|a| a == "--verify") {
        return verify::clip(std::path::Path::new(
            args.get(index + 1)
                .ok_or_else(|| anyhow::anyhow!("Provide the path to an MP4 clip"))?,
        ))
        .map(|_| true);
    }
    if args.iter().any(|a| a == "--self-test") {
        return self_test::run(args).map(|_| true);
    }

    Ok(false)
}
