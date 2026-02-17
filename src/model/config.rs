use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub editor: EditorConfig,
    pub search: SearchConfig,
    pub sync: SyncConfig,
}

#[derive(Debug, Deserialize)]
pub struct GeneralConfig {
    pub vault_path: String,
    pub scratch_file: String,
    pub auto_save_debounce_ms: u64,
    pub theme: String,
}

#[derive(Debug, Deserialize)]
pub struct EditorConfig {
    pub tab_width: u16,
    pub soft_wrap: bool,
    pub line_numbers: bool,
    pub scroll_off: u16,
}

#[derive(Debug, Deserialize)]
pub struct SearchConfig {
    pub max_results: usize,
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyncConfig {
    pub backend: String,
    pub git: Option<GitSyncConfig>,
}

#[derive(Debug, Deserialize)]
pub struct GitSyncConfig {
    pub auto_commit: bool,
    pub auto_push: bool,
    pub commit_message_format: String,
}

impl AppConfig {
    /// Load configuration with layering: defaults → user config.
    pub fn load() -> Result<Self> {
        let defaults = include_str!("../../config/default.toml");
        let mut config: AppConfig = toml::from_str(defaults)?;

        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "blackbox") {
            let config_path = proj_dirs.config_dir().join("config.toml");
            if config_path.exists() {
                let user_str = fs::read_to_string(&config_path)?;
                let user_config: AppConfig = toml::from_str(&user_str)?;
                config = user_config; // TODO: deep merge instead of full replace
            }
        }

        // Expand ~ in vault_path
        if config.general.vault_path.starts_with('~') {
            let home = dirs_home().ok_or_else(|| anyhow!("cannot determine home directory"))?;
            config.general.vault_path =
                config
                    .general
                    .vault_path
                    .replacen('~', &home.to_string_lossy(), 1);
        }

        Ok(config)
    }

    pub fn vault_path(&self) -> PathBuf {
        PathBuf::from(&self.general.vault_path)
    }

    pub fn scratch_path(&self) -> PathBuf {
        self.vault_path().join(&self.general.scratch_file)
    }
}

fn dirs_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}
