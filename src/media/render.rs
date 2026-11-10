use super::*;
use anyhow::ensure;
use std::{
    io::{BufRead, BufReader, Read},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

pub(super) fn run_process(
    mut command: Command,
    progress: &Arc<Mutex<JobProgress>>,
    cancel: &AtomicBool,
    span: (f64, f64),
) -> Result<()> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .context("Could not start the editing engine.")?;
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let state = progress.clone();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(value) = line
                .strip_prefix("out_time_us=")
                .and_then(|v| v.parse::<f64>().ok())
            {
                let mut p = state.lock().unwrap();
                p.fraction =
                    (span.0 + value / 1_000_000.0 / span.1.max(0.001)).clamp(0.0, 0.99) as f32;
            }
        }
    });
    let errors = thread::spawn(move || {
        let mut tail = Vec::new();
        let mut block = [0; 4096];
        while let Ok(n) = stderr.read(&mut block) {
            if n == 0 {
                break;
            }
            tail.extend_from_slice(&block[..n]);
            if tail.len() > 16384 {
                tail.drain(..tail.len() - 8192);
            }
        }
        String::from_utf8_lossy(&tail).into_owned()
    });
    let result = loop {
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!("Cancelled"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(e.into());
            }
        }
    };
    let _ = reader.join();
    let error = errors.join().unwrap_or_default();
    let status = result?;
    ensure!(
        status.success(),
        "Export failed. The NVIDIA encoder may be busy or unavailable. {error}"
    );
    Ok(())
}
pub(super) fn dimensions(project: &EditProject, options: &ExportOptions) -> (u32, u32, u32) {
    if let Some(canvas) = options.canvas {
        return canvas;
    }
    let first = &project.segments[0];
    let (_, _, w, h) = first.crop.pixels(&first.info);
    let height = options.height.unwrap_or(h).min(h).max(2) & !1;
    let width = ((w as f64 * height as f64 / h as f64).round() as u32).max(2) & !1;
    let fps = project
        .segments
        .iter()
        .map(|s| s.info.fps)
        .fold(60.0, f64::min)
        .round()
        .clamp(1.0, 60.0) as u32;
    (width, height, fps)
}
fn filters(s: &ClipSegment, dimensions: (u32, u32, u32)) -> String {
    let (x, y, w, h) = s.crop.pixels(&s.info);
    let (out_w, out_h, fps) = dimensions;
    let scale = (out_w as f64 / w as f64)
        .min(out_h as f64 / h as f64)
        .min(1.0);
    let sw = ((w as f64 * scale).floor() as u32).max(2) & !1;
    let sh = ((h as f64 * scale).floor() as u32).max(2) & !1;
    let mut filter = format!(
        "[0:v:0]crop={w}:{h}:{x}:{y},scale={sw}:{sh},pad={out_w}:{out_h}:(ow-iw)/2:(oh-ih)/2,fps={fps},setsar=1,format=yuv420p,setpts=PTS-STARTPTS[v];"
    );
    let mut labels = String::new();
    for (i, t) in s.audio.iter().enumerate() {
        filter.push_str(&format!("[0:{}]aresample=48000,aformat=channel_layouts=stereo,volume={},asetpts=PTS-STARTPTS[a{i}];",t.index,s.gain(t)));
        labels.push_str(&format!("[a{i}]"));
    }
    if s.audio.is_empty() {
        filter.push_str(&format!(
            "anullsrc=r=48000:cl=stereo,atrim=duration={:.6}[a]",
            s.duration()
        ));
    } else {
        filter.push_str(&format!("{labels}amix=inputs={}:normalize=0:dropout_transition=0,alimiter=limit=0.98:latency=1,apad,atrim=duration={:.6}[a]",s.audio.len(),s.duration()));
    }
    filter
}
fn bitrate(project: &EditProject, options: &ExportOptions) -> Result<u64> {
    if let Some(mb) = options.size_mb {
        ensure!(
            mb.is_finite() && (1.0..=10000.0).contains(&mb),
            "Choose a file size between 1 and 10000 MB."
        );
        let video = mb * 1_000_000.0 * 8.0 * 0.94 / project.duration() - 192_000.0;
        ensure!(
            video >= 100_000.0,
            "The file-size limit is too small for this selection. Shorten the video or increase the limit."
        );
        Ok(video.min(40_000_000.0) as u64)
    } else {
        Ok(12_000_000)
    }
}
pub(super) fn export(
    tools: &Tools,
    project: &EditProject,
    options: &ExportOptions,
    progress: &Arc<Mutex<JobProgress>>,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    project.validate()?;
    ensure!(
        !options.path.exists(),
        "A file with that name already exists. Choose another name."
    );
    let parent = options.path.parent().context("Choose an export folder.")?;
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".gavnel-export-")
        .tempdir_in(parent)?;
    let dimensions = dimensions(project, options);
    let initial = bitrate(project, options)?;
    let mut finished = None;
    for attempt in 0..3 {
        let rate = (initial as f64 * 0.8f64.powi(attempt)) as u64;
        let mut elapsed = 0.0;
        let mut list = String::new();
        for (index, s) in project.segments.iter().enumerate() {
            ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
            progress.lock().unwrap().message =
                format!("Exporting clip {} of {}", index + 1, project.segments.len());
            let filename = format!("segment-{index}.mp4");
            let path = staging.path().join(&filename);
            let mut command = tools.command(false);
            command
                .args([
                    "-hide_banner",
                    "-v",
                    "error",
                    "-nostdin",
                    "-y",
                    "-ss",
                    &format!("{:.6}", s.start),
                    "-i",
                ])
                .arg(&s.info.path)
                .args([
                    "-t",
                    &format!("{:.6}", s.duration()),
                    "-filter_complex",
                    &filters(s, dimensions),
                    "-map",
                    "[v]",
                    "-map",
                    "[a]",
                    "-c:v",
                    "h264_nvenc",
                    "-preset",
                    "p4",
                    "-b:v",
                    &rate.to_string(),
                    "-maxrate",
                    &rate.to_string(),
                    "-bufsize",
                    &(rate * 2).to_string(),
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                    "-ar",
                    "48000",
                    "-movflags",
                    "+faststart",
                    "-progress",
                    "pipe:1",
                ])
                .arg(path);
            run_process(
                command,
                progress,
                cancel,
                (elapsed / project.duration(), project.duration()),
            )?;
            elapsed += s.duration();
            list.push_str(&format!("file '{filename}'\n"));
        }
        let listing = staging.path().join("segments.txt");
        std::fs::write(&listing, list)?;
        let output = staging.path().join("finished.mp4");
        let mut command = tools.command(false);
        command
            .args([
                "-v", "error", "-nostdin", "-y", "-f", "concat", "-safe", "1", "-i",
            ])
            .arg(listing)
            .args([
                "-c",
                "copy",
                "-movflags",
                "+faststart",
                "-progress",
                "pipe:1",
            ])
            .arg(&output);
        progress.lock().unwrap().message = "Finishing export".into();
        run_process(command, progress, cancel, (0.95, project.duration() * 20.0))?;
        let info = tools.probe(&output)?;
        ensure!(
            (info.duration - project.duration()).abs() < 0.2 + project.segments.len() as f64 * 0.08,
            "Export duration validation failed."
        );
        if options.size_mb.is_none_or(|mb| {
            std::fs::metadata(&output).is_ok_and(|s| s.len() as f64 <= mb * 1_000_000.0)
        }) {
            finished = Some(output);
            break;
        }
    }
    let output = finished.context(
        "The export exceeds the requested size. Increase the limit or shorten the selection.",
    )?;
    ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
    let mut offset = 0.0;
    let mut markers = Vec::new();
    let mut mapped_events = Vec::new();
    for s in &project.segments {
        for time in crate::bookmarks::load(&s.info.path).unwrap_or_default() {
            if time >= s.start && time <= s.end {
                markers.push(offset + time - s.start);
            }
        }
        for mut event in crate::highlights::load_events(&s.info.path) {
            let seconds = event.ticks as f64 / crate::replay::TICKS as f64;
            if seconds >= s.start && seconds <= s.end {
                event.ticks = ((offset + seconds - s.start) * crate::replay::TICKS as f64) as u64;
                mapped_events.push(event);
            }
        }
        offset += s.duration();
    }
    for marker in markers {
        crate::bookmarks::append(&output, marker)?;
    }
    crate::highlights::save_events(&output, &mapped_events)?;
    let mut sidecars = Vec::new();
    let published = (|| -> Result<()> {
        for extension in ["bookmarks", "events.json"] {
            let from = output.with_extension(extension);
            if from.exists() {
                let to = options.path.with_extension(extension);
                crate::library::move_without_replacing(&from, &to)?;
                sidecars.push(to);
            }
        }
        ensure!(!cancel.load(Ordering::Acquire), "Cancelled");
        crate::library::move_without_replacing(&output, &options.path)
            .context("Could not save the export. Choose a new filename or folder.")?;
        Ok(())
    })();
    if let Err(error) = published {
        for sidecar in sidecars {
            let _ = std::fs::remove_file(sidecar);
        }
        return Err(error);
    }
    Ok(options.path.clone())
}
