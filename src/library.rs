use crossbeam_channel::{Sender, bounded};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};
#[derive(Clone)]
pub struct Clip {
    pub path: PathBuf,
    pub name: String,
    pub source: String,
    pub bytes: u64,
    pub duration: f64,
    pub modified: SystemTime,
}
impl Clip {
    pub fn is_screenshot(&self) -> bool {
        self.path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    }
}
pub struct Library {
    pub clips: Arc<Mutex<Vec<Clip>>>,
    refresh: Sender<Job>,
    pub notice: Arc<Mutex<Option<String>>>,
}
enum Job {
    Refresh(PathBuf),
    Organize(PathBuf),
}

pub fn delete_capture(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)?;
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
    {
        // The recording has already been removed. A locked sidecar can be left
        // behind without reporting that deleting the recording itself failed.
        for extension in ["bookmarks", "tracks.json", "events.json"] {
            let _ = std::fs::remove_file(path.with_extension(extension));
        }
    }
    Ok(())
}
impl Library {
    pub fn new(ctx: eframe::egui::Context, visible: Arc<std::sync::atomic::AtomicBool>) -> Self {
        let clips = Arc::new(Mutex::new(Vec::new()));
        let shared = clips.clone();
        let notice = Arc::new(Mutex::new(None));
        let shared_notice = notice.clone();
        let (tx, rx) = bounded::<Job>(4);
        std::thread::Builder::new()
            .name("gavnel-library".into())
            .spawn(move || {
                for job in rx {
                    let folder = match job {
                        Job::Refresh(folder) => folder,
                        Job::Organize(folder) => {
                            *shared_notice.lock().unwrap() = Some(match organize_existing(&folder) {
                                Ok((moved, skipped)) => format!("Organized {moved} recordings into game/source folders. {skipped} files left in place."),
                                Err(error) => format!("Could not organize recordings: {error:#}"),
                            });
                            folder
                        }
                    };
                    let result = scan(&folder);
                    *shared.lock().unwrap() = result;
                    if visible.load(std::sync::atomic::Ordering::Relaxed) {
                        ctx.request_repaint();
                    }
                }
            })
            .expect("Could not start clip library");
        Self {
            clips,
            refresh: tx,
            notice,
        }
    }
    pub fn refresh(&self, folder: &Path) {
        let _ = self.refresh.try_send(Job::Refresh(folder.to_owned()));
    }
    pub fn organize(&self, folder: &Path) -> anyhow::Result<()> {
        self.refresh
            .try_send(Job::Organize(folder.to_owned()))
            .map_err(|_| anyhow::anyhow!("The library is busy. Try organizing again shortly."))
    }
}
fn scan(folder: &Path) -> Vec<Clip> {
    let mut clips = Vec::new();
    let mut folders = vec![folder.to_owned()];
    while let Some(current) = folders.pop() {
        for entry in std::fs::read_dir(current).into_iter().flatten().flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if kind.is_symlink() || is_reparse_point(&meta) {
                continue;
            }
            if kind.is_dir() {
                folders.push(path);
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            if !path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("mp4") || x.eq_ignore_ascii_case("png"))
            {
                continue;
            }
            let duration = if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
            {
                0.0
            } else {
                std::fs::File::open(&path)
                    .ok()
                    .and_then(|f| {
                        mp4::Mp4Reader::read_header(std::io::BufReader::new(f), meta.len()).ok()
                    })
                    .map(|m| m.duration().as_secs_f64())
                    .unwrap_or(0.0)
            };
            let Some(stem) = path.file_stem() else {
                continue;
            };
            let stem = stem.to_string_lossy();
            let source = if path.parent() != Some(folder) {
                path.strip_prefix(folder)
                    .ok()
                    .and_then(|p| p.components().next())
                    .map(|p| p.as_os_str().to_string_lossy().into_owned())
            } else {
                legacy_source(&stem).map(|s| crate::export::source_folder_name(&s))
            }
            .unwrap_or_else(|| "Other clips".into());
            clips.push(Clip {
                name: stem.replace('_', " "),
                source,
                path,
                bytes: meta.len(),
                duration,
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    clips.sort_by_key(|c| std::cmp::Reverse(c.modified));
    clips
}

fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

fn legacy_source(stem: &str) -> Option<String> {
    stem.match_indices('_').find_map(|(i, _)| {
        let timestamp = stem.get(i + 1..i + 24)?;
        chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d_%H-%M-%S_%3f").ok()?;
        let source = stem[..i].trim_end_matches('_').replace('_', " ");
        (!source.trim().is_empty()).then(|| source.trim().to_owned())
    })
}

fn organize_existing(root: &Path) -> anyhow::Result<(usize, usize)> {
    let root = root.canonicalize()?;
    let mut moved = 0;
    let mut skipped = 0;
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if !entry.file_type()?.is_file()
            || is_reparse_point(&metadata)
            || !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
        {
            continue;
        }
        let source = path
            .file_stem()
            .and_then(|s| legacy_source(&s.to_string_lossy()));
        let Some(source) = source else {
            skipped += 1;
            continue;
        };
        let folder = crate::export::source_folder(&root, &source)?;
        let Some(filename) = path.file_name() else {
            skipped += 1;
            continue;
        };
        let destination = folder.join(filename);
        // Never replace an existing clip; locked/open files also remain in place.
        if move_without_replacing(&path, &destination).is_err() {
            skipped += 1;
        } else {
            let mut moved_sidecars: Vec<&str> = Vec::new();
            let mut failed = false;
            for extension in ["bookmarks", "tracks.json", "events.json"] {
                let sidecar = path.with_extension(extension);
                if sidecar.exists() {
                    if move_without_replacing(&sidecar, &destination.with_extension(extension))
                        .is_err()
                    {
                        failed = true;
                        break;
                    }
                    moved_sidecars.push(extension);
                }
            }
            if failed {
                for extension in moved_sidecars {
                    move_without_replacing(
                        &destination.with_extension(extension),
                        &path.with_extension(extension),
                    )?;
                }
                move_without_replacing(&destination, &path)?;
                skipped += 1;
                continue;
            }
            moved += 1;
        }
    }
    Ok((moved, skipped))
}

pub(crate) fn move_without_replacing(source: &Path, destination: &Path) -> anyhow::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{Win32::Storage::FileSystem::MoveFileW, core::PCWSTR};
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileW(PCWSTR(source.as_ptr()), PCWSTR(destination.as_ptr()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn deleting_a_recording_removes_its_sidecar_and_failed_delete_preserves_it() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("clip.mp4");
        crate::bookmarks::append(&video, 1.0).unwrap();
        assert!(super::delete_capture(&video).is_err());
        assert!(crate::bookmarks::path(&video).exists());
        std::fs::write(&video, b"test").unwrap();
        super::delete_capture(&video).unwrap();
        assert!(!video.exists());
        assert!(!crate::bookmarks::path(&video).exists());
    }
    #[test]
    fn saved_screenshots_appear_in_the_library_with_the_source_folder() {
        let dir = tempfile::tempdir().unwrap();
        let image = crate::screenshot::Screenshot {
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        };
        let path = crate::screenshot::save(&image, dir.path(), "Desktop", None).unwrap();
        let clips = super::scan(dir.path());
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].path, path);
        assert!(clips[0].is_screenshot());
        assert_eq!(clips[0].source, "Desktop");
    }
    use super::*;
    #[test]
    fn library_reads_legacy_and_nested_recordings_and_ignores_unfinished_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Fortnite")).unwrap();
        std::fs::create_dir_all(root.path().join("Desktop/older")).unwrap();
        for name in [
            "Rust_2026-11-08_15-00-00_123.mp4",
            "Fortnite/game.mp4",
            "Desktop/older/display.MP4",
            "Desktop/incomplete.mp4.partial",
        ] {
            std::fs::write(root.path().join(name), b"test").unwrap();
        }
        let clips = scan(root.path());
        assert_eq!(clips.len(), 3);
        assert!(clips.iter().any(|c| c.source == "Fortnite"));
        assert!(clips.iter().any(|c| c.source == "Desktop"));
        assert!(clips.iter().any(|c| c.source == "Rust"));
    }
    #[test]
    fn older_recordings_are_organized_without_overwriting_or_moving_unrelated_files() {
        let root = tempfile::tempdir().unwrap();
        let name = "Fortnite___2026-11-08_15-00-00_123.mp4";
        std::fs::write(root.path().join(name), b"original").unwrap();
        std::fs::write(
            root.path().join("Display_1_2026-11-08_15-00-00_123.mp4"),
            b"desktop",
        )
        .unwrap();
        std::fs::write(root.path().join("unrelated.mp4"), b"other").unwrap();
        assert_eq!(organize_existing(root.path()).unwrap(), (2, 1));
        assert_eq!(
            std::fs::read(root.path().join("Fortnite").join(name)).unwrap(),
            b"original"
        );
        std::fs::write(root.path().join(name), b"newer").unwrap();
        assert_eq!(organize_existing(root.path()).unwrap(), (0, 2));
        assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"newer");
        assert_eq!(
            std::fs::read(root.path().join("Fortnite").join(name)).unwrap(),
            b"original"
        );
        assert_eq!(scan(root.path()).len(), 4);
        assert_eq!(
            legacy_source("NBA_2026_2026-11-08_15-00-00_123").unwrap(),
            "NBA 2026"
        );
    }
}
