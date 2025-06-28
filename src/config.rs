use anyhow::{bail, Result};
use log::info;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_toolchain: Option<String>,

    #[serde(default)]
    pub overrides: HashMap<PathBuf, String>,

    #[serde(default)]
    pub offline: bool,

    #[serde(default)]
    pub proxy: Option<String>,
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
                info!("changed: offline = {value}");
            }
            "proxy" => {
                // TODO: check proxy address.
                self.proxy = Some(value.to_string());
                info!("changed: proxy = {value}");
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
        ret.push_str(&format!("  proxy: {}\n", self.proxy.clone().unwrap_or("null".to_string())));
        ret.fmt(f)
    }
}
