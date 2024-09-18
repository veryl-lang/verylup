use anyhow::{bail, Result};
use reqwest::Url;
use semver::Version;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::ZipArchive;

pub async fn get_latest_version(project: &str) -> Result<Version> {
    let url = format!("https://github.com/veryl-lang/{project}/releases/latest");
    let resp = reqwest::get(url).await?;
    let path = resp.url().path();
    let version = path.split("/").last().unwrap();
    let version = version.strip_prefix('v').unwrap();
    let version = Version::parse(version)?;
    Ok(version)
}

include!(concat!(env!("OUT_DIR"), "/target.rs"));

pub fn get_archive_url(project: &str, version: &Version) -> Result<Url> {
    let archive = if TARGET.starts_with("x86_64-unknown-linux") {
        format!("{project}-x86_64-linux.zip")
    } else if TARGET.starts_with("x86_64-pc-windows") {
        format!("{project}-x86_64-windows.zip")
    } else if TARGET.starts_with("x86_64-apple") {
        format!("{project}-x86_64-mac.zip")
    } else if TARGET.starts_with("aarch64-apple") {
        format!("{project}-aarch64-mac.zip")
    } else {
        bail!("unknown target");
    };

    let url =
        format!("https://github.com/veryl-lang/{project}/releases/download/v{version}/{archive}");
    let url = Url::parse(&url)?;
    Ok(url)
}

#[cfg(not(windows))]
pub fn set_exec(file: &mut File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = file.metadata()?.permissions();
    perm.set_mode(0o755);
    file.set_permissions(perm)?;
    Ok(())
}

#[cfg(windows)]
pub fn set_exec(_file: &mut File) -> Result<()> {
    Ok(())
}

pub async fn download(url: &Url) -> Result<Vec<u8>> {
    let resp = reqwest::get(url.clone()).await?;

    if !resp.status().is_success() {
        bail!("failed to download the archive: {url}");
    }

    Ok(resp.bytes().await?.to_vec())
}

pub fn unzip(file: &File, dir: &Path) -> Result<()> {
    let mut zip = ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut src = zip.by_index(i)?;
        let path = dir.join(src.name());
        let mut tgt = File::create(&path)?;
        let mut buf = Vec::new();
        src.read_to_end(&mut buf)?;
        tgt.write_all(&buf)?;
        set_exec(&mut tgt)?;
    }
    Ok(())
}
