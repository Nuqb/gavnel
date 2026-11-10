//! GitHub release discovery and user-initiated, verified installer updates.
mod controller;
mod download;
mod install;
mod metadata;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result, ensure};
pub use controller::{Controller, State};
pub use install::early;
use semver::Version;
use std::path::{Path, PathBuf};

pub const REPOSITORY: &str = "https://github.com/Nuqb/gavnel";
const API: &str = "https://api.github.com/repos/Nuqb/gavnel";
const MAX_INSTALLER: u64 = 256 * 1024 * 1024;
const INSTALL_MARKER: &str = "Gavnel GitHub release installation v1";

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    pub url: String,
    size: u64,
    digest: String,
    download_url: String,
}
#[derive(Clone, Debug)]
pub struct Prepared {
    pub release: Release,
    path: PathBuf,
}

pub fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .is_some_and(|exe| managed_executable(&exe))
}
fn managed_executable(exe: &Path) -> bool {
    exe.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("Gavnel.exe"))
        && exe.parent().is_some_and(|dir| {
            std::fs::read_to_string(dir.join("gavnel-install.txt"))
                .is_ok_and(|text| text.trim() == INSTALL_MARKER)
                && dir.join("unins000.exe").is_file()
        })
}
fn cache_dir() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("Windows user folders are unavailable")?;
    Ok(base.cache_dir().join("Gavnel").join("updates"))
}
fn agent(seconds: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(5)
        .timeout_global(Some(std::time::Duration::from_secs(seconds)))
        .build()
        .into()
}
