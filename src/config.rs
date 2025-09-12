use anyhow::{bail, Result};
use log::info;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_toolchain: Option<String>,

    #[serde(default)]
    pub overrides: HashMap<PathBuf, String>,

    #[serde(default)]
    pub offline: bool,

    #[serde(default)]
    pub proxy: Option<String>,

    #[serde(default = "default_self_update")]
    pub self_update: bool,
}

fn default_self_update() -> bool {
    true
}

impl Config {
    pub fn load() -> Self {
        let path = directories::ProjectDirs::from("com.github", "veryl-lang", "verylup")
            .map(|proj| proj.preference_dir().join("config.toml"))
            .filter(|path| path.exists());

        let Some(path) = path else {
            return Self::default();
        };

        let Ok(toml) = fs::read_to_string(path) else {
            return Self::default();
        };

        toml::from_str(&toml).unwrap_or_else(|_| Self::default())
    }

    pub fn save(&self) -> Result<()> {
        let dir = directories::ProjectDirs::from("com.github", "veryl-lang", "verylup")
            .map(|proj| proj.preference_dir().to_path_buf());

        if let Some(dir) = dir {
            if !dir.exists() {
                fs::create_dir_all(&dir)?;
            }

            let path = dir.join("config.toml");
            let toml = toml::to_string(self)?;
            fs::write(path, toml)?;
        }

        Ok(())
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "offline" => {
                let value: bool = value.parse()?;
                self.offline = value;
                info!("set: offline = {value}");
            }
            "proxy" => {
                // TODO: check proxy address.
                self.proxy = Some(value.to_string());
                info!("set: proxy = {value}");
            }
            _ => {
                bail!("Unknown key: {}", key)
            }
        }
        Ok(())
    }

    pub fn unset(&mut self, key: &str) -> Result<()> {
        match key {
            "offline" => {
                self.offline = false;
                info!("unset: offline");
            }
            "proxy" => {
                self.proxy = None;
                info!("unset: proxy");
            }
            _ => {
                bail!("Unknown key: {}", key)
            }
        }
        Ok(())
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ret = String::new();
        ret.push_str("Verylup configuration\n");
        ret.push_str(&format!("  offline: {}\n", self.offline));
        if let Some(x) = &self.proxy {
            ret.push_str(&format!("  proxy: {x}\n"));
        }
        ret.push_str(&format!("  self_update: {}\n", self.self_update));
        ret.fmt(f)
    }
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str("").unwrap_or_else(|_| Self::default())
    }
}
