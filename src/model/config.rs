use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub editor: EditorConfig,
    pub search: SearchConfig,
    #[allow(dead_code)] // Phase 3: git sync feature
    pub sync: SyncConfig,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Deserialize)]
pub struct PluginConfig {
    #[serde(default)]
    #[allow(dead_code)] // Phase 3: plugin git source
    pub repo: Option<String>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    #[allow(dead_code)] // Phase 3: plugin branch pinning
    pub branch: Option<String>,
    #[serde(default = "default_plugin_enabled")]
    pub enabled: bool,
    #[serde(default)]
    #[allow(dead_code)] // Phase 3: plugin-specific config passed to runtime
    pub config: HashMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
pub struct GeneralConfig {
    pub vault_path: String,
    pub scratch_file: String,
    pub auto_save_debounce_ms: u64,
    #[allow(dead_code)] // Phase 3: theme selection
    pub theme: String,
}

#[derive(Debug, Deserialize)]
pub struct EditorConfig {
    #[allow(dead_code)] // Phase 3: tab expansion in editor widget
    pub tab_width: u16,
    #[allow(dead_code)] // Phase 3: soft-wrap in viewport layout
    pub soft_wrap: bool,
    #[allow(dead_code)] // Phase 3: line numbers gutter
    pub line_numbers: bool,
    pub scroll_off: u16,
}

#[derive(Debug, Deserialize)]
pub struct SearchConfig {
    pub max_results: usize,
    #[allow(dead_code)] // Phase 3: pass to WalkBuilder for content search
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyncConfig {
    #[allow(dead_code)] // Phase 3: sync backend selection
    pub backend: String,
    #[allow(dead_code)] // Phase 3: git sync configuration
    pub git: Option<GitSyncConfig>,
}

#[derive(Debug, Deserialize)]
pub struct GitSyncConfig {
    #[allow(dead_code)] // Phase 3: auto-commit on save
    pub auto_commit: bool,
    #[allow(dead_code)] // Phase 3: auto-push to remote
    pub auto_push: bool,
    #[allow(dead_code)] // Phase 3: commit message template
    pub commit_message_format: String,
}

impl AppConfig {
    /// Load configuration with layering: defaults → user config (deep merge).
    ///
    /// A partial user config (e.g. only `[editor]`) safely inherits all other
    /// sections from the shipped defaults rather than discarding them.
    pub fn load() -> Result<Self> {
        let defaults_str = include_str!("../../config/default.toml");
        let mut merged: toml::Table = toml::from_str(defaults_str)?;

        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "blackbox") {
            let config_path = proj_dirs.config_dir().join("config.toml");
            if config_path.exists() {
                let user_str = fs::read_to_string(&config_path)?;
                let user_table: toml::Table = toml::from_str(&user_str)?;
                merge_tables(&mut merged, user_table);
            }
        }

        let mut config: AppConfig = toml::Value::Table(merged).try_into()?;

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

/// Recursively merge `src` into `dst`. Values in `src` override `dst`.
/// Tables are merged recursively; all other value types are replaced wholesale.
fn merge_tables(dst: &mut toml::Table, src: toml::Table) {
    for (key, src_val) in src {
        if let (Some(toml::Value::Table(dst_tbl)), toml::Value::Table(src_tbl)) =
            (dst.get_mut(&key), &src_val)
        {
            merge_tables(dst_tbl, src_tbl.clone());
        } else {
            dst.insert(key, src_val);
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}

fn default_plugin_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_defaults_no_user_config() {
        // Directly deserialise the shipped defaults — must parse without error.
        let defaults_str = include_str!("../../config/default.toml");
        let config: Result<AppConfig, _> = toml::from_str(defaults_str);
        assert!(config.is_ok(), "default config should parse: {config:?}");
        let cfg = config.unwrap();
        assert_eq!(cfg.general.scratch_file, ".scratch.md");
        assert_eq!(cfg.editor.scroll_off, 5);
        assert_eq!(cfg.search.max_results, 50);
    }

    #[test]
    fn test_tilde_expansion() {
        let mut config: AppConfig =
            toml::from_str(include_str!("../../config/default.toml")).unwrap();
        config.general.vault_path = "~/notes".to_string();

        if config.general.vault_path.starts_with('~') {
            let home = dirs_home().unwrap();
            config.general.vault_path =
                config
                    .general
                    .vault_path
                    .replacen('~', &home.to_string_lossy(), 1);
        }

        assert!(
            !config.general.vault_path.starts_with('~'),
            "tilde should be expanded, got: {}",
            config.general.vault_path
        );
        assert!(
            config.general.vault_path.contains("notes"),
            "expanded path should contain 'notes'"
        );
    }

    #[test]
    fn test_deep_merge_partial_user_config() {
        let defaults_str = include_str!("../../config/default.toml");
        let mut merged: toml::Table = toml::from_str(defaults_str).unwrap();

        // User only overrides editor.tab_width
        let user_str = "[editor]\ntab_width = 2\n";
        let user_table: toml::Table = toml::from_str(user_str).unwrap();
        merge_tables(&mut merged, user_table);

        let config: AppConfig = toml::Value::Table(merged).try_into().unwrap();

        // User override applied
        assert_eq!(config.editor.tab_width, 2, "user tab_width should win");
        // Defaults preserved for untouched fields
        assert_eq!(
            config.editor.scroll_off, 5,
            "default scroll_off should be preserved"
        );
        assert_eq!(
            config.general.scratch_file, ".scratch.md",
            "default scratch_file should be preserved"
        );
        assert_eq!(
            config.search.max_results, 50,
            "default max_results should be preserved"
        );
    }

    #[test]
    fn test_deep_merge_user_overrides_general() {
        let defaults_str = include_str!("../../config/default.toml");
        let mut merged: toml::Table = toml::from_str(defaults_str).unwrap();

        let user_str = "[general]\nvault_path = \"/custom/vault\"\nscratch_file = \".scratch.md\"\nauto_save_debounce_ms = 500\ntheme = \"cyberpunk\"\n";
        let user_table: toml::Table = toml::from_str(user_str).unwrap();
        merge_tables(&mut merged, user_table);

        let config: AppConfig = toml::Value::Table(merged).try_into().unwrap();
        assert_eq!(config.general.vault_path, "/custom/vault");
        assert_eq!(config.general.auto_save_debounce_ms, 500);
        // Editor defaults still intact
        assert_eq!(config.editor.tab_width, 4);
    }

    #[test]
    fn test_merge_tables_recursive() {
        let mut dst: toml::Table = toml::from_str("[a]\nx = 1\ny = 2\n").unwrap();
        let src: toml::Table = toml::from_str("[a]\nx = 99\n").unwrap();
        merge_tables(&mut dst, src);
        let a = dst["a"].as_table().unwrap();
        assert_eq!(a["x"].as_integer().unwrap(), 99, "src should override x");
        assert_eq!(a["y"].as_integer().unwrap(), 2, "y should be preserved");
    }
}
