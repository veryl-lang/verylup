use anyhow::Result;
use serde_derive::{Deserialize, Serialize};
use std::fs;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub default_toolchain: Option<String>,
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
}
