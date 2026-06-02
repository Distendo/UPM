use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::{Result, UpmError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub upm_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub installed_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub config_dir: PathBuf,
    pub index_url: String,
    pub github_token: Option<String>,
    pub concurrency: usize,
    pub default_prefix: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let upm_dir = home.join(".upm");

        Self {
            upm_dir: upm_dir.clone(),
            cache_dir: upm_dir.join("cache"),
            installed_dir: upm_dir.join("installed"),
            logs_dir: upm_dir.join("logs"),
            config_dir: upm_dir.join("config"),
            index_url: "https://raw.githubusercontent.com/Distendo/UPM/master/index/official.json".to_string(),
            github_token: None,
            concurrency: 4,
            default_prefix: PathBuf::from("/usr/local"),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_file_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| UpmError::ConfigError(format!("Failed to read config: {e}")))?;
            let cfg: Config = serde_json::from_str(&content)
                .map_err(|e| UpmError::ConfigError(format!("Failed to parse config: {e}")))?;
            cfg.ensure_dirs()?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            cfg.save()?;
            cfg.ensure_dirs()?;
            Ok(cfg)
        }
    }

    fn config_file_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        home.join(".upm").join("config").join("config.json")
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| UpmError::ConfigError(format!("Failed to create config dir: {e}")))?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| UpmError::ConfigError(format!("Failed to serialize config: {e}")))?;
        std::fs::write(&path, content)
            .map_err(|e| UpmError::ConfigError(format!("Failed to write config: {e}")))?;
        Ok(())
    }

    fn ensure_dirs(&self) -> Result<()> {
        for dir in [&self.upm_dir, &self.cache_dir, &self.installed_dir, &self.logs_dir, &self.config_dir] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
