use super::*;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

// Only the opt-in OCR image fixture uses an external reference generator.
// This helper is absent from the shipped application.
pub(crate) fn reference_command() -> std::process::Command {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/media/bin/ffmpeg.exe");
    std::process::Command::new(path)
}

#[test]
fn editor_works_without_external_media_tools() {
    self_test::run().unwrap();
}

#[test]
fn native_editing_preserves_sources_mixes_audio_and_maps_markers() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("Game footage \u{fc}.mp4");
    let silent = directory.path().join("silent.mp4");
    self_test::fixture(&source, true).unwrap();
    self_test::fixture(&silent, false).unwrap();
    let original = std::fs::read(&source).unwrap();
    let info = probe(&source).unwrap();
    assert_eq!(
        info.audio
            .iter()
            .map(|a| a.label.as_str())
            .collect::<Vec<_>>(),
        ["Game", "Microphone"]
    );
    let strips = filmstrip(&source).unwrap();
    assert_ne!(strips[0].pixels(), strips[7].pixels());
    // The top and bottom colors differ, exposing upside-down RGB buffers.
    let image = frame(&source, 0.5).unwrap();
    let pixels = image.pixels();
    assert!(pixels[0].b() + 80 < pixels[80 * 160].b());
    crate::bookmarks::append(&source, 0.8).unwrap();
    crate::highlights::save_events(
        &source,
        &[crate::highlights::GameEvent {
            game: crate::highlights::Game::Cs2,
            kind: crate::highlights::EventKind::Kill,
            ticks: 8_000_000,
            source: "fixture".into(),
            confidence: 1.0,
        }],
    )
    .unwrap();
    let mut first = ClipSegment::new(probe(&source).unwrap());
    first.start = 7.0 / 24.0;
    first.end = 1.5;
    first.crop.aspect = Aspect::Vertical;
    first.audio[1].muted = true;
    let mut second = ClipSegment::new(probe(&silent).unwrap());
    second.end = 0.5;
    let project = EditProject::new(vec![first.clone(), second]);
    let saved = directory.path().join("project.gavnel-project");
    project.save(&saved).unwrap();
    assert_eq!(EditProject::load(&saved).unwrap(), project);
    let output = directory.path().join("edited.mp4");
    let options = ExportOptions {
        path: output.clone(),
        height: Some(720),
        size_mb: Some(1.0),
        canvas: None,
    };
    let progress = Arc::new(Mutex::new(JobProgress::default()));
    let cancel = AtomicBool::new(false);
    render::export(&project, &options, &progress, &cancel).unwrap();
    let exported = probe(&output).unwrap();
    assert_eq!((exported.width, exported.height), (54, 96));
    assert_eq!(exported.audio.len(), 1);
    assert!((exported.duration - project.duration()).abs() < 0.15);
    assert!(std::fs::metadata(&output).unwrap().len() <= 1_000_000);
    assert_eq!(original, std::fs::read(&source).unwrap());
    let markers = crate::bookmarks::load(&output).unwrap();
    assert!((markers[0] - (0.8 - first.start)).abs() < 0.001);
    assert_eq!(crate::highlights::load_events(&output).len(), 1);
    let mut video = frames::Video::new(&first.info).unwrap();
    video.seek(first.start).unwrap();
    let expected = video
        .at(first.start)
        .unwrap()
        .render(first.crop.pixels(&first.info), (54, 96));
    let actual = frame(&output, 0.0).unwrap();
    let error = actual
        .pixels()
        .iter()
        .zip(expected.as_chunks::<4>().0)
        .map(|(a, b)| {
            (a.r() as f64 - b[2] as f64).abs()
                + (a.g() as f64 - b[1] as f64).abs()
                + (a.b() as f64 - b[0] as f64).abs()
        })
        .sum::<f64>()
        / (54.0 * 96.0 * 3.0);
    assert!(error < 18.0, "Trim/crop frame mismatch: {error}");
    let audio = crate::audio_check::decoded_frames(&output, 1).unwrap();
    let magnitude = |hz: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, sample) in audio.iter().take(24_000).enumerate() {
            let angle = std::f64::consts::TAU * hz * i as f64 / 48_000.0;
            re += sample[0] as f64 * angle.cos();
            im += sample[0] as f64 * angle.sin();
        }
        (re * re + im * im).sqrt()
    };
    assert!(
        magnitude(440.0) > magnitude(880.0) * 10.0,
        "Muted microphone leaked into export"
    );
    assert!(render::export(&project, &options, &progress, &cancel).is_err());
    let cancelled = ExportOptions {
        path: directory.path().join("cancelled.mp4"),
        ..options
    };
    assert!(render::export(&project, &cancelled, &progress, &AtomicBool::new(true)).is_err());
    assert!(!cancelled.path.exists());
    assert!(!std::fs::read_dir(directory.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".gavnel-export-")
    }));
}

#[test]
fn malformed_and_missing_videos_return_errors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.mp4");
    assert!(probe(&path).is_err());
    std::fs::write(&path, b"not a video").unwrap();
    assert!(probe(&path).is_err());
    assert!(frame(&path, 0.0).is_err());
}

#[test]
fn native_decoder_preserves_visible_edges_with_macroblock_padding() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("padded.mp4");
    let writer = encode::Writer::new(&path, (54, 54, 24), 800_000, 0).unwrap();
    let bytes: Vec<u8> = (0..54)
        .flat_map(|y| {
            (0..54).flat_map(move |_| {
                if y < 27 {
                    [10, 10, 220, 255]
                } else {
                    [220, 10, 10, 255]
                }
            })
        })
        .collect();
    for frame in 0..3 {
        writer
            .write(
                writer.video,
                &bytes,
                frame * 10_000_000 / 24..(frame + 1) * 10_000_000 / 24,
            )
            .unwrap();
    }
    writer.finish().unwrap();
    let image = frame(&path, 0.0).unwrap();
    assert_eq!(image.resolution(), [54, 54]);
    assert!(image.pixels()[0].r() > 180);
    assert!(image.pixels()[53 * 54].b() > 180);
}
