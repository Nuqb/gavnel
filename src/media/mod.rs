mod audio;
mod composition;
mod encode;
mod frames;
mod inspect;
mod model;
mod native;
mod render;
mod self_test;
#[cfg(test)]
pub(crate) mod tests;
mod worker;
use anyhow::{Context, Result, ensure};
pub use frames::{filmstrip, frame};
pub use inspect::probe;
pub use model::*;
pub use self_test::run as self_test;
use std::path::{Path, PathBuf};
pub use worker::{MediaReply, MediaService};
#[cfg(test)]
pub(crate) fn self_test_fixture(path: &Path) -> Result<()> {
    self_test::fixture(path, false)
}

pub fn preview_dimensions(project: &EditProject) -> (u32, u32, u32) {
    render::dimensions(
        project,
        &ExportOptions {
            path: PathBuf::new(),
            height: Some(720),
            size_mb: None,
            canvas: None,
        },
    )
}
