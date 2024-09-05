use anyhow::{anyhow, bail, Error, Result};
use directories::ProjectDirs;
use log::info;
use reqwest::Url;
use semver::Version;
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, PathBuf};
use std::process::Command;
use zip::ZipArchive;

pub const TOOLS: &[&str] = &["veryl", "veryl-ls"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolChain {
    Version(Version),
    Latest,
    Local,
}

impl ToolChain {
    pub fn get_actual_version(&self) -> Result<Version> {
        let path = self.get_path("veryl");
        let output = Command::new(path).arg("--version").output()?;
        let version = String::from_utf8(output.stdout)?;
        let version = version.strip_prefix("veryl ").unwrap().trim_end();
        let version = Version::parse(version)?;
        Ok(version)
    }

    pub fn get_dir(&self) -> PathBuf {
        Self::base_dir().join(self.to_string())
    }

    pub fn get_path(&self, bin: &str) -> PathBuf {
        self.get_dir().join(bin)
    }

    fn base_dir() -> PathBuf {
        let project_dir = ProjectDirs::from("org", "veryl-lang", "veryl").unwrap();
        let data_path = project_dir.data_dir().to_path_buf();
        data_path.join("toolchains")
    }

    pub fn exists(&self) -> bool {
        self.get_dir().exists()
    }

    pub fn default_toolchain() -> Option<ToolChain> {
        Self::list().last().cloned()
    }

    pub fn list() -> Vec<ToolChain> {
        let mut ret = Vec::new();

        if let Ok(dirs) = std::fs::read_dir(Self::base_dir()) {
            for dir in dirs.flatten() {
                let path = dir.path();
                let name = path.components().last();
                if let Some(Component::Normal(x)) = name {
                    if let Ok(x) = ToolChain::try_from(&x.to_string_lossy().into_owned()) {
                        ret.push(x);
                    }
                }
            }
        }

        ret.sort();
        ret
    }

    pub async fn install(&self) -> Result<()> {
        let version = match self {
            ToolChain::Latest => {
                let latest = get_latest_version().await?;
                if let Ok(actual) = self.get_actual_version() {
                    if latest != actual {
                        Some(latest)
                    } else {
                        None
                    }
                } else {
                    Some(latest)
                }
            }
            ToolChain::Version(x) => {
                if let Ok(actual) = self.get_actual_version() {
                    if *x != actual {
                        Some(x.clone())
                    } else {
                        None
                    }
                } else {
                    Some(x.clone())
                }
            }
            ToolChain::Local => {
                local_install()?;
                return Ok(());
            }
        };

        let Some(version) = version else {
            info!("checking toolchain: {self} (up-to-date)");
            return Ok(());
        };

        info!("downloading toolchain: {self}");

        let url = get_archive_url(&version)?;
        let data = download(&url).await?;
        let mut file = tempfile::tempfile()?;
        file.write_all(&data)?;

        info!("installing toolchain: {self}");

        let dir = self.get_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

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

    pub async fn uninstall(&self) -> Result<()> {
        info!("uninstalling toolchain: {self}");

        let dir = self.get_dir();
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        } else {
            bail!("toolchain \"{self}\" is not found");
        }

        Ok(())
    }
}

impl fmt::Display for ToolChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolChain::Version(x) => x.fmt(f),
            ToolChain::Latest => "latest".fmt(f),
            ToolChain::Local => "local".fmt(f),
        }
    }
}

impl TryFrom<&str> for ToolChain {
    type Error = Error;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "latest" => Ok(ToolChain::Latest),
            "local" => Ok(ToolChain::Local),
            x => {
                let version = Version::parse(x);
                if let Ok(version) = version {
                    Ok(ToolChain::Version(version))
                } else {
                    Err(anyhow!("unknown toolchain \"{value}\""))
                }
            }
        }
    }
}

impl TryFrom<&String> for ToolChain {
    type Error = Error;
    fn try_from(value: &String) -> std::result::Result<Self, Self::Error> {
        ToolChain::try_from(value.as_str())
    }
}

impl Ord for ToolChain {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (ToolChain::Local, ToolChain::Local) => std::cmp::Ordering::Equal,
            (ToolChain::Local, _) => std::cmp::Ordering::Greater,
            (ToolChain::Latest, ToolChain::Local) => std::cmp::Ordering::Less,
            (ToolChain::Latest, ToolChain::Latest) => std::cmp::Ordering::Equal,
            (ToolChain::Latest, _) => std::cmp::Ordering::Greater,
            (ToolChain::Version(_), ToolChain::Local) => std::cmp::Ordering::Less,
            (ToolChain::Version(_), ToolChain::Latest) => std::cmp::Ordering::Less,
            (ToolChain::Version(x), ToolChain::Version(y)) => x.cmp(y),
        }
    }
}

impl PartialOrd for ToolChain {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub async fn get_latest_version() -> Result<Version> {
    let resp = reqwest::get("https://github.com/veryl-lang/veryl/releases/latest").await?;
    let path = resp.url().path();
    let version = path.split("/").last().unwrap();
    let version = version.strip_prefix('v').unwrap();
    let version = Version::parse(version)?;
    Ok(version)
}

include!(concat!(env!("OUT_DIR"), "/target.rs"));

fn get_archive_url(version: &Version) -> Result<Url> {
    let archive = if TARGET.starts_with("x86_64-unknown-linux") {
        "veryl-x86_64-linux.zip"
    } else if TARGET.starts_with("x86_64-pc-windows") {
        "veryl-x86_64-windows.zip"
    } else if TARGET.starts_with("x86_64-apple") {
        "veryl-x86_64-mac.zip"
    } else if TARGET.starts_with("aarch64-apple") {
        "veryl-aarch64-mac.zip"
    } else {
        bail!("unknown target");
    };

    let url = format!("https://github.com/veryl-lang/veryl/releases/download/v{version}/{archive}");
    let url = Url::parse(&url)?;
    Ok(url)
}

#[cfg(not(windows))]
fn set_exec(file: &mut File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = file.metadata()?.permissions();
    perm.set_mode(0o755);
    file.set_permissions(perm)?;
    Ok(())
}

#[cfg(windows)]
fn set_exec(_file: &mut File) -> Result<()> {
    Ok(())
}

async fn download(url: &Url) -> Result<Vec<u8>> {
    let resp = reqwest::get(url.clone()).await?;

    if !resp.status().is_success() {
        bail!("failed to download the archive of toolchain: {url}");
    }

    Ok(resp.bytes().await?.to_vec())
}

fn local_install() -> Result<()> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .output()?;
    let output = String::from_utf8(output.stdout)?;
    let metadata: serde_json::Value = serde_json::from_str(&output)?;

    let temp = tempfile::tempdir()?;
    let root = temp.path();
    let bin = root.join("bin");

    let mut built = false;

    let env_path = if let Some(path) = std::env::var_os("PATH") {
        let mut paths: Vec<_> = std::env::split_paths(&path).collect();
        paths.push(bin.clone());
        std::env::join_paths(paths)?
    } else {
        bail!("");
    };

    for pkg in metadata["packages"].as_array().unwrap() {
        let name = pkg["name"].as_str().unwrap();
        if TOOLS.contains(&name) {
            let manifest = PathBuf::from(pkg["manifest_path"].as_str().unwrap());
            let path = manifest.parent().unwrap();

            info!("building local toolchain: {name}");

            let mut child = Command::new("cargo")
                .arg("install")
                .arg("--path")
                .arg(path)
                .arg("--root")
                .arg(root)
                .env("PATH", &env_path)
                .spawn()?;

            child.wait()?;
            built = true;
        }
    }

    if !built {
        bail!("this is not Veryl's repository");
    }

    let dir = ToolChain::Local.get_dir();

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    for file in fs::read_dir(bin)? {
        let file = file?;
        let tgt = dir.join(file.file_name());
        fs::copy(file.path(), &tgt)?;
    }

    Ok(())
}
