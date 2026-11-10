use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
    state: String,
}

pub(super) fn latest() -> Result<Option<Release>> {
    let mut response = agent(20)
        .get(format!("{API}/releases/latest"))
        .header("User-Agent", concat!("Gavnel/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .context("Could not check GitHub releases. Check your connection and try again.")?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_vec()?;
    parse(&bytes, &Version::parse(env!("CARGO_PKG_VERSION"))?)
}

pub(super) fn parse(bytes: &[u8], current: &Version) -> Result<Option<Release>> {
    let release: GithubRelease =
        serde_json::from_slice(bytes).context("Invalid GitHub release response")?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )?;
    if !version.pre.is_empty() || version <= *current {
        return Ok(None);
    }
    ensure!(
        release.tag_name == format!("v{version}"),
        "Unexpected release tag format"
    );
    let name = format!("Gavnel-Setup-{version}-x64.exe");
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == name)
        .context("This release does not have a Windows x64 installer yet")?;
    let download_url = format!("{REPOSITORY}/releases/download/v{version}/{name}");
    ensure!(
        asset.browser_download_url == download_url,
        "Unexpected installer download location"
    );
    ensure!(
        asset.state == "uploaded" && asset.size > 2 && asset.size <= MAX_INSTALLER,
        "The release installer is incomplete or too large"
    );
    let digest = asset
        .digest
        .and_then(|digest| digest.strip_prefix("sha256:").map(str::to_owned))
        .context("GitHub has not provided a SHA-256 digest for this installer")?;
    ensure!(
        digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid installer checksum"
    );
    Ok(Some(Release {
        url: format!("{REPOSITORY}/releases/tag/v{version}"),
        version,
        size: asset.size,
        digest: digest.to_lowercase(),
        download_url,
    }))
}
