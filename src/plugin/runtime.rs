use std::fs;
use std::path::{Path, PathBuf};

use crate::plugin::manifest::{PluginId, PluginManifest};

#[derive(Debug, Clone)]
pub enum PluginStatus {
    Discovered,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct PluginRuntime {
    pub id: PluginId,
    pub root_dir: PathBuf,
    pub manifest: Option<PluginManifest>,
    pub status: PluginStatus,
}

impl PluginRuntime {
    pub fn discover(id: PluginId, root_dir: PathBuf) -> Self {
        match Self::read_manifest(&root_dir) {
            Ok(manifest) => Self {
                id,
                root_dir,
                manifest: Some(manifest),
                status: PluginStatus::Discovered,
            },
            Err(err) => Self {
                id,
                root_dir,
                manifest: None,
                status: PluginStatus::Error(err),
            },
        }
    }

    pub fn status(&self) -> &PluginStatus {
        &self.status
    }

    pub fn supports_command(&self, command: &str) -> bool {
        self.manifest
            .as_ref()
            .map(|manifest| manifest.commands.iter().any(|cmd| cmd.name == command))
            .unwrap_or(false)
    }

    pub fn execute_command(&mut self, command: &str) -> Result<Option<String>, String> {
        if !self.supports_command(command) {
            return Ok(None);
        }

        self.ensure_loaded()?;

        Ok(Some(format!(
            "plugin {} handled command: {}",
            self.display_name(),
            command
        )))
    }

    pub fn display_name(&self) -> String {
        self.manifest
            .as_ref()
            .map(|manifest| manifest.name.clone())
            .unwrap_or_else(|| self.id.0.clone())
    }

    fn read_manifest(root_dir: &Path) -> Result<PluginManifest, String> {
        let manifest_path = root_dir.join("plugin.toml");
        let raw = fs::read_to_string(&manifest_path)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))?;

        toml::from_str::<PluginManifest>(&raw)
            .map_err(|err| format!("{}: {err}", manifest_path.display()))
    }

    fn ensure_loaded(&mut self) -> Result<(), String> {
        if matches!(self.status, PluginStatus::Loaded) {
            return Ok(());
        }

        let manifest = self
            .manifest
            .as_ref()
            .ok_or_else(|| "missing plugin manifest".to_string())?;

        let wasm_path = self.root_dir.join(&manifest.entry);
        if !wasm_path.is_file() {
            let err = format!("missing wasm entry: {}", wasm_path.display());
            self.status = PluginStatus::Error(err.clone());
            return Err(err);
        }

        self.status = PluginStatus::Loaded;
        Ok(())
    }
}
