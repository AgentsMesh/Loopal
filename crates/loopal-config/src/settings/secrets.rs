use serde::{Deserialize, Serialize};

/// Vault subsystem settings. All fields optional — sensible defaults apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SecretsSettings {
    /// Override path to the vaults directory. When `None`, auto-discovery looks
    /// for `<cwd>/.loopal/vaults/` (and ancestors).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vaults_dir: Option<std::path::PathBuf>,

    /// Override which vault is "default" (the one CLI / runtime treats as
    /// highest priority). When `None`, the name `"default"` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_vault: Option<String>,
}
