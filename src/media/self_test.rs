//! Explicit, isolated editor diagnostics; never opens capture devices or plays audio.
use super::{encode::Writer, native::TICKS, *};
use std::sync::{Arc, Mutex, atomic::AtomicBool};

pub(super) fn fixture(path: &Path, audio: bool) -> Result<()> {
    let writer = Writer::new(path, (160, 96, 24), 800_000, if audio { 3 } else { 0 })?;
    for frame in 0..48 {
        let time = frame as f64 / 24.0;
        let mut pixels = Vec::with_capacity(160 * 96 * 4);
        for y in 0..96 {
            for x in 0..160 {
                pixels.extend_from_slice(&[
                    if y < 48 { 10 } else { 200 },
                    (x as u8).saturating_add(20),
                    (frame * 4) as u8,
                    255,
                ]);
            }
        }
        let range = (time * TICKS).round() as i64..((time + 1.0 / 24.0) * TICKS).round() as i64;
        writer.write(writer.video, &pixels, range.clone())?;
        for (track, stream) in writer.audio.iter().enumerate() {
            let pcm: Vec<u8> = (0..2000)
                .flat_map(|sample| {
                    let t = (frame * 2000 + sample) as f64 / 48_000.0;
                    let tone = |hz: f64| (std::f64::consts::TAU * hz * t).sin() * 0.2;
                    let value = match track {
                        0 => tone(440.0) + tone(880.0),
                        1 => tone(440.0),
                        _ => tone(880.0),
                    };
                    let value = (value * 32767.0) as i16;
                    [value, value].into_iter().flat_map(i16::to_le_bytes)
                })
                .collect();
            writer.write(*stream, &pcm, range.clone())?;
        }
    }
    writer.finish()?;
    if audio {
        std::fs::write(
            path.with_extension("tracks.json"),
            r#"["Combined","Game","Microphone"]"#,
        )?;
    }
    Ok(())
}

pub fn run() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("Editor footage ü.mp4");
    fixture(&source, true)?;
    let info = probe(&source)?;
    ensure!(
        (info.width, info.height) == (160, 96) && info.audio.len() == 2,
        "Native video inspection failed."
    );
    ensure!(filmstrip(&source)?.len() == 8, "Timeline decoding failed.");
    let mut segment = ClipSegment::new(info);
    segment.start = 0.5;
    segment.end = 1.5;
    segment.crop.aspect = Aspect::Vertical;
    segment.audio[1].muted = true;
    let project = EditProject::new(vec![segment]);
    let output = directory.path().join("edited.mp4");
    let options = ExportOptions {
        path: output.clone(),
        height: None,
        size_mb: Some(1.0),
        canvas: None,
    };
    render::export(
        &project,
        &options,
        &Arc::new(Mutex::new(JobProgress::default())),
        &AtomicBool::new(false),
    )?;
    let info = probe(&output)?;
    ensure!(
        (info.width, info.height) == (54, 96) && (info.duration - 1.0).abs() < 0.15,
        "Native export validation failed."
    );
    ensure!(
        frame(&output, 0.0)?.resolution() == [54, 96],
        "Native preview decoding failed."
    );
    Ok(())
}
