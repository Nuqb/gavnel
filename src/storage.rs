use anyhow::{Context, Result};
use std::path::Path;
use windows::{
    Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumePathNameW},
    core::PCWSTR,
};

pub struct DriveSpace {
    pub root: String,
    pub total: u64,
    pub free: u64,
}
pub fn drive_space(folder: &Path) -> Result<DriveSpace> {
    let existing = folder
        .ancestors()
        .find(|path| path.exists())
        .context("Storage drive is unavailable")?;
    let wide: Vec<u16> = existing
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut root = [0u16; 1024];
    let mut total = 0;
    let mut free = 0;
    unsafe {
        GetVolumePathNameW(PCWSTR(wide.as_ptr()), &mut root)
            .context("Could not identify the storage drive")?;
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            None,
            Some(&mut total),
            Some(&mut free),
        )
        .context("Could not read available drive space")?;
    }
    let end = root.iter().position(|c| *c == 0).unwrap_or(root.len());
    Ok(DriveSpace {
        root: String::from_utf16_lossy(&root[..end]),
        total,
        free,
    })
}
pub fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if value < 1000.0 || unit == "TB" {
            return format!("{value:.2} {unit}");
        }
        value /= 1000.0;
    }
    unreachable!()
}
pub fn folder_bytes(folder: &Path) -> Result<u64> {
    if !folder.exists() {
        return Ok(0);
    }
    let mut total = 0u64;
    let mut folders = vec![folder.to_owned()];
    while let Some(path) = folders.pop() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                continue;
            } // Do not follow junctions or links.
            if metadata.is_dir() {
                folders.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    Ok(total)
}
pub fn check_limit(folder: &Path, limit: Option<u32>, additional: u64) -> Result<()> {
    if let Some(gb) = limit {
        anyhow::ensure!(
            folder_bytes(folder)?.saturating_add(additional) < u64::from(gb) * 1_000_000_000,
            "Local storage limit reached. Choose a larger limit or another capture folder."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_nested_files_and_blocks_writes_at_limit_without_removing_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Game")).unwrap();
        std::fs::write(dir.path().join("Game/clip.mp4"), [0; 100]).unwrap();
        std::fs::write(dir.path().join("session.partial"), [0; 50]).unwrap();
        assert_eq!(folder_bytes(dir.path()).unwrap(), 150);
        assert!(check_limit(dir.path(), Some(1), 1_000_000_000 - 150).is_err());
        assert!(check_limit(dir.path(), None, u64::MAX).is_ok());
        assert_eq!(folder_bytes(dir.path()).unwrap(), 150);
        let space = drive_space(dir.path()).unwrap();
        assert!(space.total >= space.free && space.total > 0);
    }
}
