use anyhow::Result;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub fn path(video: &Path) -> PathBuf {
    video.with_extension("bookmarks")
}

pub fn append(video: &Path, seconds: f64) -> Result<()> {
    anyhow::ensure!(
        seconds.is_finite() && seconds >= 0.0,
        "Invalid bookmark time"
    );
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path(video))?;
    writeln!(file, "{seconds:.6}")?;
    file.sync_data()?;
    Ok(())
}

pub fn load(video: &Path) -> Result<Vec<f64>> {
    let text = match std::fs::read_to_string(path(video)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut times: Vec<f64> = text
        .lines()
        .filter_map(|line| line.parse::<f64>().ok())
        .filter(|time| time.is_finite() && *time >= 0.0)
        .collect();
    times.sort_by(f64::total_cmp);
    Ok(times)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bookmarks_persist_and_ignore_invalid_entries() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("session.mp4");
        assert!(load(&video).unwrap().is_empty());
        append(&video, 123.5).unwrap();
        append(&video, 4.25).unwrap();
        assert!(append(&video, f64::NAN).is_err());
        std::fs::OpenOptions::new()
            .append(true)
            .open(path(&video))
            .unwrap()
            .write_all(b"NaN\n-1\nbroken\n")
            .unwrap();
        assert_eq!(load(&video).unwrap(), vec![4.25, 123.5]);
    }
}
