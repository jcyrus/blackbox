use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::model::config::AppConfig;
use crate::model::config::PluginConfig;
use crate::plugin::manifest::PluginId;
use crate::plugin::runtime::PluginRuntime;
use crate::plugin::runtime::PluginStatus;

#[derive(Debug, Default)]
pub struct PluginManager {
    runtimes: HashMap<PluginId, PluginRuntime>,
}

impl PluginManager {
    pub fn new(config: &AppConfig) -> Self {
        let mut manager = Self::default();

        for plugin in &config.plugins {
            if !plugin.enabled {
                continue;
            }

            let Some(root_dir) = Self::resolve_plugin_root(plugin) else {
                continue;
            };

            let plugin_id = PluginId::new(Self::plugin_key(plugin, &root_dir));
            manager
                .runtimes
                .entry(plugin_id.clone())
                .or_insert_with(|| PluginRuntime::discover(plugin_id, root_dir));
        }

        manager
    }

    pub fn plugin_count(&self) -> usize {
        self.runtimes.len()
    }

    pub fn error_count(&self) -> usize {
        self.runtimes
            .values()
            .filter(|runtime| matches!(runtime.status(), PluginStatus::Error(_)))
            .count()
    }

    pub fn startup_notifications(&self) -> Vec<String> {
        if self.runtimes.is_empty() {
            return Vec::new();
        }

        let mut notices = vec![self.summary_notification()];
        notices.extend(self.error_notifications());
        notices
    }

    pub fn summary_notification(&self) -> String {
        let discovered = self.plugin_count().saturating_sub(self.error_count());
        format!(
            "plugins: {discovered} discovered, {} errors",
            self.error_count()
        )
    }

    pub fn error_notifications(&self) -> Vec<String> {
        self.runtimes
            .values()
            .filter_map(|runtime| {
                if let PluginStatus::Error(err) = runtime.status() {
                    Some(format!(
                        "plugin {} ({}): {err}",
                        runtime.display_name(),
                        runtime.root_dir.display()
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn list_notifications(&self) -> Vec<String> {
        if self.runtimes.is_empty() {
            return vec!["plugins: none configured".to_string()];
        }

        let mut rows: Vec<String> = self
            .runtimes
            .values()
            .map(|runtime| {
                let status = match runtime.status() {
                    PluginStatus::Discovered => "discovered".to_string(),
                    PluginStatus::Loaded => "loaded".to_string(),
                    PluginStatus::Error(err) => format!("error: {err}"),
                };

                format!(
                    "plugin {} [{status}] ({})",
                    runtime.display_name(),
                    runtime.root_dir.display()
                )
            })
            .collect();

        rows.sort();
        rows
    }

    pub fn command_notifications(&self) -> Vec<String> {
        let mut commands: Vec<String> = self
            .runtimes
            .values()
            .filter_map(|runtime| runtime.manifest.as_ref())
            .flat_map(|manifest| manifest.commands.iter().map(|command| command.name.clone()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        if commands.is_empty() {
            return vec!["plugin commands: none discovered".to_string()];
        }

        commands.sort();
        vec![format!("plugin commands: {}", commands.join(", "))]
    }

    pub fn execute_command(&mut self, command: &str) -> Vec<String> {
        let mut matches = self
            .runtimes
            .iter_mut()
            .filter(|(_, runtime)| runtime.supports_command(command));

        let Some((_, runtime)) = matches.next() else {
            return vec![format!("plugin command not found: {command}")];
        };

        if matches.next().is_some() {
            return vec![format!(
                "plugin command is ambiguous: {command} (multiple plugins)"
            )];
        }

        match runtime.execute_command(command) {
            Ok(Some(message)) => vec![message],
            Ok(None) => vec![format!("plugin command not found: {command}")],
            Err(err) => vec![format!("plugin {}: {err}", runtime.display_name())],
        }
    }

    fn resolve_plugin_root(plugin: &PluginConfig) -> Option<PathBuf> {
        if let Some(path) = plugin.path.as_ref() {
            return Some(expand_tilde(path));
        }

        plugin
            .repo
            .as_ref()
            .map(|repo| default_plugin_base_dir().join(repo_slug(repo)))
    }

    fn plugin_key(plugin: &PluginConfig, root_dir: &PathBuf) -> String {
        if let Some(repo) = plugin.repo.as_ref() {
            return format!("repo:{repo}");
        }

        if let Some(path) = plugin.path.as_ref() {
            return format!("path:{}", expand_tilde(path).display());
        }

        format!("path:{}", root_dir.display())
    }
}

fn default_plugin_base_dir() -> PathBuf {
    if let Some(project_dirs) = directories::ProjectDirs::from("", "", "blackbox") {
        return project_dirs.config_dir().join("plugins");
    }

    if let Some(base_dirs) = directories::BaseDirs::new() {
        return base_dirs.home_dir().join(".config/blackbox/plugins");
    }

    PathBuf::from(".blackbox-plugins")
}

fn repo_slug(repo: &str) -> String {
    let trimmed = repo.trim_end_matches('/').trim_end_matches(".git");
    trimmed
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("plugin")
        .to_string()
}

fn expand_tilde(path: &std::path::Path) -> PathBuf {
    let text = path.to_string_lossy();
    if !text.starts_with('~') {
        return path.to_path_buf();
    }

    if let Some(base_dirs) = directories::BaseDirs::new() {
        let home = base_dirs.home_dir().to_string_lossy();
        return PathBuf::from(text.replacen('~', &home, 1));
    }

    path.to_path_buf()
}
