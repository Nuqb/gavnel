use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub(super) fn prepare(release: Release, progress: Arc<AtomicU64>) -> Result<Prepared> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory = cache_dir()?.join(format!("{}-{nonce}", release.version));
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("Gavnel-Setup.exe");
    let result = transfer(&release, &path, &progress)
        .and_then(|()| verify(&path, &release.digest, release.size));
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result?;
    Ok(Prepared { release, path })
}

fn transfer(release: &Release, path: &Path, progress: &AtomicU64) -> Result<()> {
    let mut response = agent(300)
        .get(&release.download_url)
        .header("User-Agent", concat!("Gavnel/", env!("CARGO_PKG_VERSION")))
        .call()?;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(release.size + 1)
        .reader();
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut buffer = [0_u8; 65536];
    let mut received = 0;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        received += count as u64;
        ensure!(
            received <= release.size,
            "Installer download exceeded the expected size"
        );
        file.write_all(&buffer[..count])?;
        progress.store(received, Ordering::Relaxed);
    }
    ensure!(
        received == release.size,
        "Installer download was interrupted. Please try again."
    );
    file.sync_all()?;
    Ok(())
}

pub(super) fn verify(path: &Path, expected: &str, expected_size: u64) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() == expected_size && expected_size <= MAX_INSTALLER,
        "Installer size verification failed"
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65536];
    let mut signature = [0; 2];
    file.read_exact(&mut signature)?;
    ensure!(
        &signature == b"MZ",
        "The download is not a Windows installer"
    );
    hasher.update(signature);
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    ensure!(
        format!("{:x}", hasher.finalize()) == expected,
        "Installer checksum verification failed. Nothing was installed."
    );
    Ok(())
}
