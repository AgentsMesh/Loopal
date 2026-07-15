use std::path::Path;

use loopal_config::{LayerSource, ManagedSkill};
use loopal_ipc::protocol::methods::{
    DesktopPluginSummary, DesktopPluginsResponse, DesktopSkillDetail, DesktopSkillScope,
    DesktopSkillsResponse,
};

use super::desktop_skill_projection::{append, public_description, source_for};

pub(super) fn list(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
) -> Result<DesktopSkillsResponse, String> {
    let effective = loopal_config::load_config_with_user_dir(root, user_dir)
        .map_err(|error| error.to_string())?;
    let mut skills = Vec::new();
    for plugin in
        loopal_config::list_plugins_from_user_dir(user_dir).map_err(|error| error.to_string())?
    {
        let source = LayerSource::Plugin(plugin.name.clone());
        append(
            &mut skills,
            documents(&user_dir.join("plugins").join(&plugin.name).join("skills"))?,
            source,
            DesktopSkillScope::Plugin,
            &effective,
        );
    }
    append(
        &mut skills,
        documents(&user_dir.join("skills"))?,
        LayerSource::Global,
        DesktopSkillScope::Global,
        &effective,
    );
    append(
        &mut skills,
        documents(&root.join(".loopal/skills"))?,
        LayerSource::Project,
        DesktopSkillScope::Project,
        &effective,
    );
    skills.sort_by(|left, right| {
        (!left.editable)
            .cmp(&!right.editable)
            .then_with(|| (!left.effective).cmp(&!right.effective))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.source.cmp(&right.source))
    });
    skills.truncate(2_048);
    Ok(DesktopSkillsResponse {
        workspace_id,
        skills,
    })
}

pub(super) fn get(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
    name: &str,
) -> Result<DesktopSkillDetail, String> {
    let document =
        loopal_config::get_global_skill(user_dir, name).map_err(|error| error.to_string())?;
    detail(root, user_dir, workspace_id, document)
}

pub(super) fn upsert(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
    name: &str,
    description: &str,
    body: &str,
    expected_revision: Option<&str>,
) -> Result<DesktopSkillDetail, String> {
    let document =
        loopal_config::upsert_global_skill(user_dir, name, description, body, expected_revision)
            .map_err(|error| error.to_string())?;
    detail(root, user_dir, workspace_id, document)
}

pub(super) fn delete(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
    name: &str,
    expected_revision: &str,
) -> Result<DesktopSkillsResponse, String> {
    loopal_config::delete_global_skill(user_dir, name, expected_revision)
        .map_err(|error| error.to_string())?;
    list(root, user_dir, workspace_id)
}

pub(super) fn plugins(
    user_dir: &Path,
    workspace_id: String,
) -> Result<DesktopPluginsResponse, String> {
    let plugins = loopal_config::list_plugins_from_user_dir(user_dir)
        .map_err(|error| error.to_string())?
        .into_iter()
        .take(1_024)
        .map(|mut plugin| {
            plugin.skills.truncate(1_024);
            plugin.mcp_servers.truncate(1_024);
            DesktopPluginSummary {
                source: format!("plugin:{}", plugin.name),
                name: plugin.name,
                skills: plugin.skills,
                mcp_servers: plugin.mcp_servers,
                hook_count: plugin.hook_count.min(65_535),
                has_instructions: plugin.has_instructions,
                has_memory: plugin.has_memory,
                has_settings: plugin.has_settings,
            }
        })
        .collect();
    Ok(DesktopPluginsResponse {
        workspace_id,
        plugins,
    })
}

fn detail(
    root: &Path,
    user_dir: &Path,
    workspace_id: String,
    document: ManagedSkill,
) -> Result<DesktopSkillDetail, String> {
    let effective = loopal_config::load_config_with_user_dir(root, user_dir)
        .map_err(|error| error.to_string())?;
    let is_effective = source_for(&effective, &document.skill.name) == Some(&LayerSource::Global);
    Ok(DesktopSkillDetail {
        workspace_id,
        name: document.skill.name,
        description: public_description(&document.skill.description),
        has_arguments: document.skill.has_arg,
        source: "global".into(),
        scope: DesktopSkillScope::Global,
        editable: true,
        effective: is_effective,
        body: document.skill.body,
        revision: document.revision,
    })
}

fn documents(dir: &Path) -> Result<Vec<ManagedSkill>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    loopal_config::list_skill_documents(dir).map_err(|error| error.to_string())
}
