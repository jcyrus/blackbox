use crate::model::config::PluginConfig;

#[derive(Debug, Default)]
#[allow(dead_code)] // Phase 3 scaffolding: installer is invoked by future :PluginSync command.
pub struct PluginInstaller;

impl PluginInstaller {
    #[allow(dead_code)] // Phase 3 scaffolding: repo sync flow is planned, signature is stabilized now.
    pub fn sync(_plugins: &[PluginConfig]) {
        // Implementation will clone/pull plugin repositories and validate manifests.
    }
}
