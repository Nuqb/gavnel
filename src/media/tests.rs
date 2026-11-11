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
    let mut batches = Vec::new();
    let strips =
        frames::filmstrip_progressive(&source, |images| batches.push(images.len())).unwrap();
    assert_eq!(batches, (1..=8).collect::<Vec<_>>());
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
        replace_source: false,
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
        replace_source: false,
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

#[test]
fn save_current_video_replaces_source_and_cancellation_preserves_it() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("current.mp4");
    self_test::fixture(&source, true).unwrap();
    crate::bookmarks::append(&source, 0.8).unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut segment = ClipSegment::new(probe(&source).unwrap());
    segment.start = 0.5;
    segment.end = 1.5;
    let project = EditProject::new(vec![segment]);
    let options = ExportOptions {
        replace_source: true,
        path: source.clone(),
        height: None,
        size_mb: None,
        canvas: None,
    };
    let progress = Arc::new(Mutex::new(JobProgress::default()));
    assert!(render::export(&project, &options, &progress, &AtomicBool::new(true)).is_err());
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(crate::bookmarks::load(&source).unwrap(), vec![0.8]);
    render::export(&project, &options, &progress, &AtomicBool::new(false)).unwrap();
    assert!((probe(&source).unwrap().duration - 1.0).abs() < 0.15);
    assert_ne!(std::fs::read(&source).unwrap(), original);
    assert!((crate::bookmarks::load(&source).unwrap()[0] - 0.3).abs() < 0.001);
    assert!(!source.with_extension("tracks.json").exists());
    assert!(!std::fs::read_dir(directory.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".gavnel-export-")
    }));
    let other = directory.path().join("other.mp4");
    std::fs::write(&other, b"unrelated").unwrap();
    let options = ExportOptions {
        path: other.clone(),
        ..options
    };
    assert!(render::export(&project, &options, &progress, &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read(other).unwrap(), b"unrelated");
}

#[test]
fn export_size_estimate_tracks_duration_and_selected_budget() {
    let info = MediaInfo {
        path: PathBuf::from("example.mp4"),
        duration: 40.0,
        width: 1920,
        height: 1080,
        fps: 60.0,
        audio: vec![],
        markers: vec![],
    };
    let mut project = EditProject::new(vec![ClipSegment::new(info)]);
    let automatic = estimated_size_mb(&project, None).unwrap();
    assert!((60.0..63.0).contains(&automatic));
    let limited = estimated_size_mb(&project, Some(25.0)).unwrap();
    assert!((23.0..25.0).contains(&limited));
    project.segments[0].end = 20.0;
    assert!((estimated_size_mb(&project, None).unwrap() - automatic / 2.0).abs() < 0.001);
    assert!(estimated_size_mb(&project, Some(0.0)).is_err());
}

#[test]
fn compressed_trim_preserves_frames_audio_and_exact_visible_duration() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    self_test::fixture(&source, true).unwrap();
    let mut segment = ClipSegment::new(probe(&source).unwrap());
    segment.start = 0.0;
    segment.end = 1.0;
    let project = EditProject::new(vec![segment]);
    let output = directory.path().join("copied.mp4");
    let progress = Arc::new(Mutex::new(JobProgress::default()));
    let started = std::time::Instant::now();
    trim::write(&project, &output, &progress, &AtomicBool::new(false)).unwrap();
    let copy_elapsed = started.elapsed();
    let info = probe(&output).unwrap();
    assert!((info.duration - 1.0).abs() < 0.04, "{}", info.duration);
    assert_eq!(info.audio, project.segments[0].info.audio);
    for time in [0.0, 0.4, 0.9] {
        let actual = frame(&output, time).unwrap();
        let expected = frame(&source, time).unwrap();
        let error = actual
            .pixels()
            .iter()
            .zip(expected.pixels())
            .map(|(a, b)| (a.r() as f64 - b.r() as f64).abs())
            .sum::<f64>()
            / actual.pixels().len() as f64;
        assert!(error < 4.0, "Trim frame at {time} is wrong: {error}");
    }
    let rendered = directory.path().join("rendered.mp4");
    let started = std::time::Instant::now();
    composition::Render {
        project: &project,
        dimensions: (160, 96, 24),
        bitrate: 12_000_000,
        progress: &progress,
        cancel: &AtomicBool::new(false),
    }
    .write(&rendered)
    .unwrap();
    eprintln!(
        "Trim benchmark (160x96, 1s selection): copy={copy_elapsed:?}, render={:?}",
        started.elapsed()
    );
}

#[test]
fn fast_trim_supports_new_and_current_video_and_falls_back_for_non_keyframes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    self_test::fixture(&source, false).unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut segment = ClipSegment::new(probe(&source).unwrap());
    segment.end = 1.0;
    let mut project = EditProject::new(vec![segment]);
    let output = directory.path().join("new.mp4");
    let options = ExportOptions {
        path: output.clone(),
        replace_source: false,
        height: None,
        size_mb: None,
        canvas: None,
    };
    let progress = Arc::new(Mutex::new(JobProgress::default()));
    render::export(&project, &options, &progress, &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let samples = |path: &Path| {
        let file = std::fs::File::open(path).unwrap();
        let size = file.metadata().unwrap().len();
        let mut reader = mp4::Mp4Reader::read_header(std::io::BufReader::new(file), size).unwrap();
        reader.read_sample(1, 1).unwrap().unwrap().bytes
    };
    assert_eq!(
        samples(&source),
        samples(&output),
        "Fast trim must preserve compressed video bytes"
    );
    project.segments[0].start = 0.5;
    project.segments[0].end = 1.5;
    assert!(
        trim::write(
            &project,
            &directory.path().join("unsafe.mp4"),
            &progress,
            &AtomicBool::new(false)
        )
        .is_err()
    );
    let fallback = ExportOptions {
        path: directory.path().join("exact.mp4"),
        ..options.clone()
    };
    render::export(&project, &fallback, &progress, &AtomicBool::new(false)).unwrap();
    let expected = frame(&source, 0.5).unwrap();
    let actual = frame(&fallback.path, 0.0).unwrap();
    let error = expected
        .pixels()
        .iter()
        .zip(actual.pixels())
        .map(|(a, b)| (a.r() as f64 - b.r() as f64).abs())
        .sum::<f64>()
        / actual.pixels().len() as f64;
    assert!(
        error < 6.0,
        "Exact fallback trim changed its start frame: {error}"
    );
    project.segments[0].start = 0.0;
    project.segments[0].end = 0.5;
    let replacement = ExportOptions {
        path: source.clone(),
        replace_source: true,
        ..options
    };
    render::export(&project, &replacement, &progress, &AtomicBool::new(false)).unwrap();
    assert!((probe(&source).unwrap().duration - 0.5).abs() < 0.04);
    assert_eq!(samples(&source), samples(&output));
}
