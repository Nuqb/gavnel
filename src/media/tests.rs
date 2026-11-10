use super::*;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
fn fixture(tools: &Tools, path: &Path, silent: bool) {
    let mut cmd = tools.command(false);
    cmd.args([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        if silent {
            "color=c=blue:size=480x360:rate=24:duration=2"
        } else {
            "testsrc2=size=640x360:rate=30:duration=4"
        },
    ]);
    if !silent {
        cmd.args([
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=4",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:sample_rate=48000:duration=4",
            "-filter_complex",
            "[1:a][2:a]amix=inputs=2[mix]",
            "-map",
            "0:v",
            "-map",
            "[mix]",
            "-map",
            "1:a",
            "-map",
            "2:a",
        ]);
    }
    let output = cmd
        .args(["-c:v", "h264_nvenc", "-g", "90", "-c:a", "aac"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if !silent {
        std::fs::write(
            path.with_extension("tracks.json"),
            r#"["Combined","Game","Microphone"]"#,
        )
        .unwrap();
    }
}
#[test]
#[ignore = "requires pinned media runtime and NVIDIA encoder; generates synthetic files only"]
fn editing_pipeline_preserves_sources_and_checks_real_media() {
    let tools = Tools::locate().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("work/editor-tests");
    std::fs::create_dir_all(&root).unwrap();
    let dir = tempfile::tempdir_in(root).unwrap();
    let source = dir.path().join("Game footage ünicode.mp4");
    let silent = dir.path().join("silent.mp4");
    fixture(&tools, &source, false);
    fixture(&tools, &silent, true);
    let filmstrip = tools.filmstrip(&source).unwrap();
    assert_eq!(filmstrip.len(), 8);
    assert!(filmstrip.iter().all(|f| f.resolution()[0] == 144));
    assert_ne!(filmstrip[0].pixels(), filmstrip[7].pixels());
    let original = std::fs::read(&source).unwrap();
    let info = tools.probe(&source).unwrap();
    assert_eq!(info.audio.len(), 2);
    assert_eq!(info.audio[0].label, "Game");
    assert_eq!(info.audio[1].label, "Microphone");
    crate::bookmarks::append(&source, 1.2).unwrap();
    crate::highlights::save_events(
        &source,
        &[crate::highlights::GameEvent {
            game: crate::highlights::Game::Cs2,
            kind: crate::highlights::EventKind::Kill,
            ticks: 12_000_000,
            source: "fixture".into(),
            confidence: 1.0,
        }],
    )
    .unwrap();
    let mut first = ClipSegment::new(tools.probe(&source).unwrap());
    first.start = 31.0 / 30.0;
    first.end = 3.2;
    first.crop.aspect = Aspect::Vertical;
    first.audio[1].muted = true;
    let project = EditProject::new(vec![first, ClipSegment::new(tools.probe(&silent).unwrap())]);
    let path = dir.path().join("project.gavnel-project");
    project.save(&path).unwrap();
    project.save(&path).unwrap();
    assert_eq!(EditProject::load(&path).unwrap(), project);
    let output = dir.path().join("edited.mp4");
    let options = ExportOptions {
        path: output.clone(),
        height: Some(720),
        size_mb: Some(10.0),
        canvas: None,
    };
    let progress = Arc::new(Mutex::new(JobProgress::default()));
    let cancel = AtomicBool::new(false);
    super::render::export(&tools, &project, &options, &progress, &cancel).unwrap();
    let rendered = tools.probe(&output).unwrap();
    assert_eq!((rendered.width, rendered.height), (202, 360));
    assert_eq!(rendered.audio.len(), 1);
    assert!((rendered.duration - project.duration()).abs() < 0.25);
    assert!(std::fs::metadata(&output).unwrap().len() <= 10_000_000);
    assert_eq!(original, std::fs::read(&source).unwrap());
    let first_frame = tools.frame(&output, 0.0).unwrap();
    let expected = tools
        .command(false)
        .args(["-v", "error", "-ss", &format!("{:.6}", 31.0 / 30.0), "-i"])
        .arg(&source)
        .args([
            "-vf",
            "crop=202:360:218:0",
            "-frames:v",
            "1",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(expected.status.success());
    let actual: Vec<u8> = first_frame
        .pixels()
        .iter()
        .flat_map(|p| p.to_array())
        .collect();
    let error = actual
        .iter()
        .zip(&expected.stdout)
        .map(|(a, b)| (*a as f64 - *b as f64).abs())
        .sum::<f64>()
        / actual.len() as f64;
    assert!(error < 15.0, "Trim/crop first-frame mismatch: {error}");
    let markers = crate::bookmarks::load(&output).unwrap();
    assert!((markers[0] - (1.2 - 31.0 / 30.0)).abs() < 0.001);
    assert_eq!(crate::highlights::load_events(&output).len(), 1);
    let audio = tools
        .command(false)
        .args(["-v", "error", "-i"])
        .arg(&output)
        .args([
            "-t", "0.6", "-f", "f32le", "-ac", "1", "-ar", "48000", "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(audio.status.success());
    let samples: Vec<f64> = audio
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b) as f64)
        .collect();
    let magnitude = |frequency: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, s) in samples.iter().enumerate() {
            let a = std::f64::consts::TAU * frequency * i as f64 / 48000.0;
            re += s * a.cos();
            im += s * a.sin();
        }
        (re * re + im * im).sqrt()
    };
    assert!(
        magnitude(440.0) > magnitude(880.0) * 10.0,
        "Muted microphone leaked into the export"
    );
    assert!(super::render::export(&tools, &project, &options, &progress, &cancel).is_err());
    assert_eq!(original, std::fs::read(&source).unwrap());
    let cancelled = ExportOptions {
        path: dir.path().join("cancelled.mp4"),
        height: None,
        size_mb: None,
        canvas: None,
    };
    assert!(
        super::render::export(
            &tools,
            &project,
            &cancelled,
            &progress,
            &AtomicBool::new(true)
        )
        .is_err()
    );
    assert!(!cancelled.path.exists());
    assert!(!std::fs::read_dir(dir.path()).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".gavnel-export-")
    }));
}
