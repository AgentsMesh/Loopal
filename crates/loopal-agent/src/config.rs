use std::collections::HashMap;
use std::path::Path;

use loopal_tool_api::PermissionMode;

/// Configuration for a sub-agent type, loaded from `.loopal/agents/*.md`.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    /// System prompt body (everything after the frontmatter).
    pub system_prompt: String,
    pub permission_mode: PermissionMode,
    pub allowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: "General-purpose agent".to_string(),
            system_prompt: String::new(),
            permission_mode: PermissionMode::Bypass,
            allowed_tools: None,
            model: None,
        }
    }
}

/// Load all agent configs from global + project directories.
/// Project configs override global configs with the same name.
pub fn load_agent_configs(cwd: &Path) -> HashMap<String, AgentConfig> {
    let mut map = HashMap::new();

    // Global agents dir: ~/.loopal/agents/
    if let Ok(global_dir) = loopal_config::global_agents_dir() {
        load_configs_from_dir(&global_dir, &mut map);
    }

    // Project agents dir: <cwd>/.loopal/agents/
    let project_dir = loopal_config::project_agents_dir(cwd);
    load_configs_from_dir(&project_dir, &mut map);

    map
}

fn load_configs_from_dir(dir: &Path, map: &mut HashMap<String, AgentConfig>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let config = parse_agent_config(stem, &content);
        map.insert(stem.to_string(), config);
    }
}

/// Parse an agent `.md` file with YAML-like frontmatter.
fn parse_agent_config(name: &str, content: &str) -> AgentConfig {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return AgentConfig {
            name: name.to_string(),
            system_prompt: content.to_string(),
            ..Default::default()
        };
    }

    let Some(first_nl) = trimmed.find('\n') else {
        return AgentConfig {
            name: name.to_string(),
            system_prompt: content.to_string(),
            ..Default::default()
        };
    };
    let after_open = &trimmed[first_nl + 1..];

    let (fm_block, body) = if let Some(end) = after_open.find("\n---") {
        let rest = &after_open[end + 4..];
        (&after_open[..end], rest.strip_prefix('\n').unwrap_or(rest))
    } else {
        return AgentConfig {
            name: name.to_string(),
            system_prompt: content.to_string(),
            ..Default::default()
        };
    };

    let mut config = AgentConfig {
        name: name.to_string(),
        system_prompt: body.to_string(),
        ..Default::default()
    };

    for line in fm_block.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("description:") {
            config.description = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("permission_mode:") {
            config.permission_mode = parse_perm_mode(val.trim());
        } else if let Some(val) = line.strip_prefix("allowed_tools:") {
            config.allowed_tools = Some(parse_list(val.trim()));
        } else if let Some(val) = line.strip_prefix("model:") {
            let v = val.trim();
            if !v.is_empty() {
                config.model = Some(v.to_string());
            }
        }
    }

    config
}

fn parse_perm_mode(s: &str) -> PermissionMode {
    match s {
        "bypass" | "yolo" => PermissionMode::Bypass,
        "supervised" | "default" | "accept-edits" => PermissionMode::Supervised,
        _ => PermissionMode::Bypass,
    }
}

/// Parse a YAML-like inline list: `[Read, Glob, Grep]`
fn parse_list(s: &str) -> Vec<String> {
    let s = s.trim_start_matches('[').trim_end_matches(']');
    s.split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_config_with_frontmatter() {
        let content = r#"---
description: Read-only explorer
permission_mode: accept-edits
allowed_tools: [Read, Glob, Grep, Ls]
model: claude-sonnet-4-20250514
---
You are a code exploration agent.
"#;
        let config = parse_agent_config("explorer", content);
        assert_eq!(config.name, "explorer");
        assert_eq!(config.description, "Read-only explorer");
        assert_eq!(
            config.allowed_tools.as_ref().unwrap(),
            &["Read", "Glob", "Grep", "Ls"]
        );
        assert!(config.system_prompt.contains("code exploration"));
    }

    #[test]
    fn test_parse_agent_config_no_frontmatter() {
        let config = parse_agent_config("simple", "Just a prompt.");
        assert_eq!(config.name, "simple");
        assert_eq!(config.system_prompt, "Just a prompt.");
        assert!(config.allowed_tools.is_none());
    }
}
