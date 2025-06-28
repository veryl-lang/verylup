use crate::config::Config;
use crate::utils::*;
use anyhow::{anyhow, bail, Error, Result};
use directories::ProjectDirs;
use log::info;
use semver::Version;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, PathBuf};
use std::process::Command;

pub const TOOLS: &[&str] = &["veryl", "veryl-ls"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolChain {
    Version(Version),
    Latest,
    Nightly,
    Local,
}

impl ToolChain {
    pub fn get_actual_version(&self) -> Result<Version> {
        let path = if cfg!(target_os = "windows") {
            self.get_path("veryl.exe")
        } else {
            self.get_path("veryl")
        };

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
        let config = Config::load();

        // directory override
        let project = search_project();
        if let Ok(project) = project {
            if let Some(x) = config.overrides.get(&project) {
                if let Some(x) = Self::by_name(x) {
                    return Some(x);
                }
            }
        }

        // default toolchain config
        if let Some(x) = config.default_toolchain {
            if let Some(x) = Self::by_name(&x) {
                return Some(x);
            }
        }

        Self::list().last().cloned()
    }

    pub fn list() -> Vec<ToolChain> {
        let mut ret = Vec::new();

        if let Ok(dirs) = std::fs::read_dir(Self::base_dir()) {
            for dir in dirs.flatten() {
                let path = dir.path();
                let name = path.components().next_back();
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

    pub fn by_name(name: &str) -> Option<ToolChain> {
        let path = Self::base_dir().join(name);

        if path.exists() {
            ToolChain::try_from(name).ok()
        } else {
            None
        }
    }

    pub async fn install(&self, pkg: &Option<PathBuf>, debug: bool, config: &Config) -> Result<()> {
        let file = if let Some(pkg) = pkg {
            info!("extracting toolchain package: {}", pkg.to_string_lossy());

            let pkg_version = get_package_version(pkg)?;

            if let Ok(actual) = self.get_actual_version() {
                if pkg_version <= actual {
                    info!("checking toolchain: {self} (up-to-date)");
                    return Ok(());
                }
            }

            if let ToolChain::Version(x) = self {
                if *x != pkg_version {
                    bail!("unexpected package: package version is {pkg_version}");
                }
            }

            File::open(pkg)?
        } else {
            let version = match self {
                ToolChain::Latest => {
                    let latest = get_latest_version("veryl", config).await?;
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
                    local_install(debug)?;
                    return Ok(());
                }
                ToolChain::Nightly => None,
            };

            let url = if self == &ToolChain::Nightly {
                get_nightly_url()?
            } else {
                let Some(version) = version else {
                    info!("checking toolchain: {self} (up-to-date)");
                    return Ok(());
                };
                get_archive_url("veryl", &version)?
            };

            info!("downloading toolchain: {self}");

            let data = download(&url, config).await?;
            let mut file = tempfile::tempfile()?;
            file.write_all(&data)?;
            file
        };

        info!("installing toolchain: {self}");

        let dir = self.get_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        unzip(&file, &dir)?;

        Ok(())
    }

    pub fn uninstall(&self) -> Result<()> {
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
            ToolChain::Nightly => "nightly".fmt(f),
        }
    }
}

impl TryFrom<&str> for ToolChain {
    type Error = Error;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "latest" => Ok(ToolChain::Latest),
            "local" => Ok(ToolChain::Local),
            "nightly" => Ok(ToolChain::Nightly),
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
            (ToolChain::Nightly, ToolChain::Local) => std::cmp::Ordering::Less,
            (ToolChain::Nightly, ToolChain::Nightly) => std::cmp::Ordering::Equal,
            (ToolChain::Nightly, _) => std::cmp::Ordering::Greater,
            (ToolChain::Latest, ToolChain::Local) => std::cmp::Ordering::Less,
            (ToolChain::Latest, ToolChain::Nightly) => std::cmp::Ordering::Less,
            (ToolChain::Latest, ToolChain::Latest) => std::cmp::Ordering::Equal,
            (ToolChain::Latest, _) => std::cmp::Ordering::Greater,
            (ToolChain::Version(_), ToolChain::Local) => std::cmp::Ordering::Less,
            (ToolChain::Version(_), ToolChain::Nightly) => std::cmp::Ordering::Less,
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

fn local_install(debug: bool) -> Result<()> {
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

    let is_veryl = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| TOOLS.contains(&x["name"].as_str().unwrap()));

    if !is_veryl {
        bail!("this is not Veryl's repository");
    }

    info!("building local toolchain");

    let (build_args, target_path) = if debug {
        (vec!["build"], "debug")
    } else {
        let toml = PathBuf::from(metadata["workspace_root"].as_str().unwrap()).join("Cargo.toml");
        let toml = fs::read_to_string(toml)?;
        if toml.contains("[profile.release-verylup]") {
            (
                vec!["build", "--profile", "release-verylup"],
                "release-verylup",
            )
        } else {
            (vec!["build", "--release"], "release")
        }
    };

    let revision = Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--format=\"%h\"")
        .output()?;
    let revision = String::from_utf8(revision.stdout).unwrap();
    let revision = revision.trim_matches(['"', '\n']);
    let date = chrono::Local::now();
    let version = metadata["packages"][0]["version"].as_str().unwrap();
    let version = format!(
        "{}-local ({} {})",
        version,
        revision,
        date.format("%Y-%m-%d")
    );

    let mut child = Command::new("cargo")
        .args(build_args)
        .env("VERSION", version)
        .spawn()?;
    child.wait()?;

    if !bin.exists() {
        fs::create_dir_all(&bin)?;
    }

    let target = PathBuf::from(metadata["target_directory"].as_str().unwrap());
    let target = target.join(target_path);

    for tool in TOOLS {
        let src = target.join(tool);
        let dst = bin.join(tool);
        fs::copy(&src, &dst)?;
    }

    let dir = ToolChain::Local.get_dir();

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    for file in fs::read_dir(bin)? {
        let file = file?;
        let tgt = dir.join(file.file_name());
        let mut tmp = dir.join(file.file_name()).clone();
        tmp.set_extension("new");

        // Copy new binary to a temporary file on the same filesystem as target, and move it to target
        // This is a workaround to avoid "Text file busy" error caused by copying to executing files.
        fs::copy(file.path(), &tmp)?;
        fs::rename(&tmp, &tgt)?;
    }

    Ok(())
}
