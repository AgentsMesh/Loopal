use std::{collections::HashSet, path::Path};

use loopal_config::{LocalSettingsFieldPatch, McpServerConfig, patch_local_settings_fields};
use loopal_ipc::protocol::methods::{DesktopMcpServerInput, DesktopMcpServersResponse};

use super::{
    desktop_mcp_layers, desktop_mcp_projection, desktop_mcp_secret_patches,
    desktop_mcp_secret_policy, desktop_mcp_validation,
};

pub(super) fn load(root: &Path, workspace_id: String) -> Result<DesktopMcpServersResponse, String> {
    let servers = desktop_mcp_layers::all(root)?
        .into_iter()
        .filter_map(|(name, (source, config))| {
            desktop_mcp_projection::project(name, source, config)
        })
        .collect();
    Ok(DesktopMcpServersResponse {
        workspace_id,
        servers,
    })
}

pub(super) fn upsert(
    root: &Path,
    workspace_id: String,
    server: DesktopMcpServerInput,
) -> Result<DesktopMcpServersResponse, String> {
    let mut patches = inherited_secret_patches(root, &server)?;
    patches.extend(desktop_mcp_validation::patches(server)?);
    patch_local_settings_fields(root, patches).map_err(|error| error.to_string())?;
    load(root, workspace_id)
}

pub(super) fn delete(
    root: &Path,
    workspace_id: String,
    name: String,
) -> Result<DesktopMcpServersResponse, String> {
    desktop_mcp_validation::validate_server_name(&name)?;
    let prefix = format!("mcp_servers.{name}");
    patch_local_settings_fields(
        root,
        [
            LocalSettingsFieldPatch::Set(format!("{prefix}.type"), "stdio".into()),
            LocalSettingsFieldPatch::Set(format!("{prefix}.command"), "__loopal_disabled__".into()),
            LocalSettingsFieldPatch::Set(format!("{prefix}.enabled"), false.into()),
            LocalSettingsFieldPatch::Remove(format!("{prefix}.url")),
            LocalSettingsFieldPatch::Remove(format!("{prefix}.args")),
            LocalSettingsFieldPatch::Remove(format!("{prefix}.env")),
            LocalSettingsFieldPatch::Remove(format!("{prefix}.headers")),
            LocalSettingsFieldPatch::Remove(format!("{prefix}.cwd_isolation")),
        ],
    )
    .map_err(|error| error.to_string())?;
    load(root, workspace_id)
}

fn inherited_secret_patches(
    root: &Path,
    input: &DesktopMcpServerInput,
) -> Result<Vec<LocalSettingsFieldPatch>, String> {
    let (name, target, enabled, explicit) = match input {
        DesktopMcpServerInput::Stdio {
            name,
            enabled,
            secret_patches,
            ..
        } => (
            name,
            "env",
            *enabled,
            secret_patches
                .iter()
                .map(|patch| patch.name.clone())
                .collect::<HashSet<_>>(),
        ),
        DesktopMcpServerInput::StreamableHttp {
            name,
            enabled,
            secret_patches,
            ..
        } => (
            name,
            "headers",
            *enabled,
            secret_patches
                .iter()
                .map(|patch| patch.name.to_ascii_lowercase())
                .collect(),
        ),
    };
    let current = desktop_mcp_layers::effective(root, name)?;
    let Some((source, config)) = current else {
        return Ok(Vec::new());
    };
    let values = match (&config, target) {
        (McpServerConfig::Stdio { env, .. }, "env") => env,
        (McpServerConfig::StreamableHttp { headers, .. }, "headers") => headers,
        _ => return Ok(Vec::new()),
    };
    if let McpServerConfig::StreamableHttp { headers, .. } = &config {
        let names = match input {
            DesktopMcpServerInput::StreamableHttp { secret_patches, .. } => secret_patches
                .iter()
                .map(|patch| patch.name.clone())
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        if enabled || !names.is_empty() {
            desktop_mcp_secret_patches::validate_header_edit(headers, &names)?;
        }
    }
    desktop_mcp_secret_policy::inherited_patches(name, target, enabled, &source, values, &explicit)
}
