use crate::config::Config;
use anyhow::{anyhow, bail, Context, Result};
use reqwest::{Response, Url};
use semver::Version;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

async fn get_url(url: &Url, config: &Config) -> Result<Response, reqwest::Error> {
    let builder = reqwest::ClientBuilder::new();
    let builder = match &config.proxy {
        Some(proxy) => builder.proxy(reqwest::Proxy::all(proxy)?),
        None => builder,
    };
    let client = builder.build()?;
    client.get(url.clone()).send().await
}

pub async fn get_latest_version(project: &str, config: &Config) -> Result<Version> {
    let url =
        Url::parse(format!("https://github.com/veryl-lang/{project}/releases/latest").as_str())
            .expect("Url error");
    let resp = get_url(&url, config).await?;
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
    } else if TARGET.starts_with("aarch64-unknown-linux") {
        format!("{project}-aarch64-linux.zip")
    } else if TARGET.starts_with("x86_64-pc-windows") {
        format!("{project}-x86_64-windows.zip")
    } else if TARGET.starts_with("aarch64-pc-windows") {
        format!("{project}-aarch64-windows.zip")
    } else if TARGET.starts_with("x86_64-apple") {
        format!("{project}-x86_64-mac.zip")
    } else if TARGET.starts_with("aarch64-apple") {
        format!("{project}-aarch64-mac.zip")
    } else {
        bail!("unknown target :{TARGET}");
    };

    let url =
        format!("https://github.com/veryl-lang/{project}/releases/download/v{version}/{archive}");
    let url = Url::parse(&url)?;
    Ok(url)
}

pub fn get_nightly_url() -> Result<Url> {
    let archive = if TARGET.starts_with("x86_64-unknown-linux") {
        "veryl-x86_64-linux.zip"
    } else if TARGET.starts_with("aarch64-unknown-linux") {
        "veryl-aarch64-linux.zip"
    } else if TARGET.starts_with("x86_64-pc-windows") {
        "veryl-x86_64-windows.zip"
    } else if TARGET.starts_with("aarch64-pc-windows") {
        "veryl-aarch64-windows.zip"
    } else if TARGET.starts_with("x86_64-apple") {
        "veryl-x86_64-mac.zip"
    } else if TARGET.starts_with("aarch64-apple") {
        "veryl-aarch64-mac.zip"
    } else {
        bail!("unknown target :{TARGET}");
    };

    let url = format!("https://static.veryl-lang.org/toolchain/nightly/{archive}");
    let url = Url::parse(&url)?;
    Ok(url)
}

pub fn get_nightly_version_url() -> Result<Url> {
    let url = "https://static.veryl-lang.org/toolchain/nightly/version";
    let url = Url::parse(url)?;
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

pub async fn download(url: &Url, config: &Config) -> Result<Vec<u8>> {
    let resp = get_url(url, config).await?;

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
        let path_str = path.to_string_lossy();
        let mut tgt = File::create(&path).with_context(|| format!("creating {path_str}"))?;
        let mut buf = Vec::new();
        src.read_to_end(&mut buf)?;
        tgt.write_all(&buf)
            .with_context(|| format!("writing {path_str}"))?;
        set_exec(&mut tgt).with_context(|| format!("setting permission of {path_str}"))?;
    }
    Ok(())
}

pub fn search_project() -> Result<PathBuf> {
    let dir = std::env::current_dir()?;
    for p in dir.ancestors() {
        if p.join("Veryl.toml").exists() {
            return Ok(dir);
        }
    }
    Err(anyhow!("Veryl project is not found"))
}

pub fn get_package_version(path: &Path) -> Result<Version> {
    let temp = tempfile::tempdir()?;
    let file = File::open(path)?;
    unzip(&file, temp.path())?;

    let path = if cfg!(target_os = "windows") {
        temp.path().join("veryl.exe")
    } else {
        temp.path().join("veryl")
    };

    let output = Command::new(path).arg("--version").output()?;
    let version = String::from_utf8(output.stdout)?;
    let version = version.strip_prefix("veryl ").unwrap().trim_end();
    let version = Version::parse(version)?;
    Ok(version)
}
