use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::plugin::permission::Permission;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub String);

impl PluginId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Phase 3 scaffolding: read after manifest discovery/runtime loading is implemented.
pub struct PluginManifest {
    pub name: String,
    #[allow(dead_code)]
    pub version: String,
    #[allow(dead_code)]
    pub entry: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    #[serde(default)]
    pub commands: Vec<CommandDef>,
    #[serde(default)]
    pub keybindings: Vec<KeybindingDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Phase 3 scaffolding: consumed by command palette registration.
pub struct CommandDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Phase 3 scaffolding: consumed by keybinding dispatcher integration.
pub struct KeybindingDef {
    pub mode: String,
    pub keys: String,
    pub action: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Phase 3 scaffolding: installer/discovery model reserved for next slice.
pub struct PluginInstallSpec {
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub config: HashMap<String, toml::Value>,
}

#[allow(dead_code)] // Phase 3 scaffolding: default deserialization helper for install specs.
fn default_enabled() -> bool {
    true
}
