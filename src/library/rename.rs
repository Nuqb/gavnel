use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

/// Rename a capture and its sidecars without replacing existing files.
pub fn rename_capture(source: &Path, name: &str) -> Result<PathBuf> {
    let destination = destination(source, name)?;
    ensure!(source.is_file(), "This capture is no longer available.");
    if source == destination {
        return Ok(destination);
    }
    let moves = planned_moves(source, &destination)?;
    let mut completed: Vec<&(PathBuf, PathBuf)> = Vec::new();
    for pair in &moves {
        if let Err(error) = super::move_without_replacing(&pair.0, &pair.1) {
            for (old, new) in completed.into_iter().rev() {
                super::move_without_replacing(new, old).with_context(|| {
                    format!(
                        "Could not restore {}. The file remains at {}.",
                        old.display(),
                        new.display()
                    )
                })?;
            }
            return Err(error.context(
                "Could not rename this capture. Close other apps using it and try again.",
            ));
        }
        completed.push(pair);
    }
    Ok(destination)
}

fn destination(source: &Path, name: &str) -> Result<PathBuf> {
    ensure!(!name.trim().is_empty(), "Enter a name.");
    ensure!(
        !name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
            && !name.ends_with(['.', ' '])
            && name != "."
            && name != "..",
        "Use a valid file name without slashes or a trailing dot or space."
    );
    ensure!(!reserved_name(name), "This name is reserved by Windows.");
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .context("Missing capture extension")?;
    ensure!(
        extension.eq_ignore_ascii_case("mp4") || extension.eq_ignore_ascii_case("png"),
        "Unsupported capture type."
    );
    let filename = format!("{name}.{extension}");
    ensure!(
        filename.encode_utf16().count() <= 255,
        "This name is too long."
    );
    Ok(source.with_file_name(filename))
}

fn reserved_name(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_uppercase();
    matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        base.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

fn planned_moves(source: &Path, destination: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut moves = vec![(source.to_owned(), destination.to_owned())];
    if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
    {
        for extension in ["bookmarks", "tracks.json", "events.json"] {
            let old = source.with_extension(extension);
            let new = destination.with_extension(extension);
            ensure_available(&old, &new)?;
            if old.try_exists()? {
                moves.push((old, new));
            }
        }
    }
    ensure_available(source, destination)?;
    Ok(moves)
}

fn ensure_available(source: &Path, destination: &Path) -> Result<()> {
    if destination.try_exists()? {
        // Windows resolves case-only renames to the same existing file.
        let same_file =
            source.try_exists()? && source.canonicalize()? == destination.canonicalize()?;
        ensure!(
            same_file,
            "A capture or companion file with this name already exists."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_preserves_capture_sidecars_and_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("old.mp4");
        std::fs::write(&source, b"video").unwrap();
        for extension in ["bookmarks", "tracks.json", "events.json"] {
            std::fs::write(source.with_extension(extension), extension).unwrap();
        }
        let target = rename_capture(&source, "My highlight").unwrap();
        assert_eq!(target, dir.path().join("My highlight.mp4"));
        assert_eq!(std::fs::read(&target).unwrap(), b"video");
        assert!(!source.exists());
        for extension in ["bookmarks", "tracks.json", "events.json"] {
            assert_eq!(
                std::fs::read_to_string(target.with_extension(extension)).unwrap(),
                extension
            );
            assert!(!source.with_extension(extension).exists());
        }
        let image = dir.path().join("old.PNG");
        std::fs::write(&image, b"image").unwrap();
        assert_eq!(
            rename_capture(&image, "Screenshot")
                .unwrap()
                .extension()
                .unwrap(),
            "PNG"
        );
    }

    #[test]
    fn conflicts_and_invalid_names_leave_original_files_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("old.mp4");
        std::fs::write(&source, b"original").unwrap();
        for name in [
            "",
            " ",
            "../escape",
            "a\\b",
            "bad?name",
            "name.",
            "name ",
            "NUL",
            "con.txt",
            "COM1",
            "LPT²",
        ] {
            assert!(rename_capture(&source, name).is_err(), "{name}");
        }
        let target = dir.path().join("new.mp4");
        std::fs::write(&target, b"existing").unwrap();
        assert!(rename_capture(&source, "new").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"existing");
        std::fs::remove_file(&target).unwrap();
        std::fs::write(target.with_extension("bookmarks"), b"unrelated").unwrap();
        assert!(rename_capture(&source, "new").is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
        assert!(!target.exists());
    }

    #[test]
    fn locked_sidecar_rolls_back_capture_and_previously_moved_sidecars() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("old.mp4");
        std::fs::write(&source, b"original").unwrap();
        std::fs::write(source.with_extension("bookmarks"), b"markers").unwrap();
        std::fs::write(source.with_extension("tracks.json"), b"tracks").unwrap();
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(source.with_extension("tracks.json"))
            .unwrap();
        assert!(rename_capture(&source, "new").is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
        assert_eq!(
            std::fs::read(source.with_extension("bookmarks")).unwrap(),
            b"markers"
        );
        assert!(!dir.path().join("new.mp4").exists());
    }

    #[test]
    fn unchanged_and_case_only_names_preserve_data() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("clip.mp4");
        std::fs::write(&source, b"original").unwrap();
        assert_eq!(rename_capture(&source, "clip").unwrap(), source);
        let target = rename_capture(&source, "Clip").unwrap();
        assert_eq!(target.file_name().unwrap(), "Clip.mp4");
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .any(|e| e.unwrap().file_name() == "Clip.mp4")
        );
    }
}
