//! Publish a rendered replacement only after rendering and validation succeed.
use super::*;

pub(super) fn replace_video(output: &Path, target: &Path) -> Result<()> {
    let backup = tempfile::Builder::new()
        .prefix(".gavnel-sidecar-recovery-")
        .tempdir_in(target.parent().context("Missing video folder")?)?;
    let mut saved = Vec::new();
    let mut installed = Vec::new();
    let result = (|| -> Result<()> {
        for extension in ["bookmarks", "events.json", "tracks.json"] {
            let destination = target.with_extension(extension);
            if destination.exists() {
                std::fs::rename(&destination, backup.path().join(extension))?;
                saved.push(extension);
            }
            let source = output.with_extension(extension);
            if source.exists() {
                crate::library::move_without_replacing(&source, &destination)?;
                installed.push(extension);
            }
        }
        replace_file(output, target).context(
            "Could not replace the current video. Close other apps using it and try again.",
        )
    })();
    if result.is_err()
        && let Err(error) = restore_sidecars(target, backup.path(), &installed, &saved)
    {
        let recovery = backup.keep();
        return Err(error.context(format!(
            "Original video unchanged; recover its sidecars from {}",
            recovery.display()
        )));
    }
    result
}

fn restore_sidecars(
    target: &Path,
    backup: &Path,
    installed: &[&str],
    saved: &[&str],
) -> Result<()> {
    for extension in installed {
        std::fs::remove_file(target.with_extension(extension))?;
    }
    for extension in saved {
        std::fs::rename(backup.join(extension), target.with_extension(extension))?;
    }
    Ok(())
}

fn replace_file(source: &Path, target: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locked_video_preserves_original_and_rolls_back_sidecars() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("current.mp4");
        let output = dir.path().join("finished.mp4");
        std::fs::write(&target, b"original").unwrap();
        std::fs::write(&output, b"edited").unwrap();
        std::fs::write(target.with_extension("bookmarks"), b"old markers").unwrap();
        std::fs::write(output.with_extension("bookmarks"), b"new markers").unwrap();
        std::fs::write(target.with_extension("tracks.json"), b"old tracks").unwrap();
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&target)
            .unwrap();
        assert!(replace_video(&output, &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        assert_eq!(
            std::fs::read(target.with_extension("bookmarks")).unwrap(),
            b"old markers"
        );
        assert_eq!(
            std::fs::read(target.with_extension("tracks.json")).unwrap(),
            b"old tracks"
        );
    }
}
