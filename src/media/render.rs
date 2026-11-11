use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

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
        let output = staging.path().join("finished.mp4");
        if output.exists() {
            std::fs::remove_file(&output)?;
        }
        super::composition::Render {
            project,
            dimensions,
            bitrate: rate,
            progress,
            cancel,
        }
        .write(&output)?;
        let info = super::probe(&output)?;
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
