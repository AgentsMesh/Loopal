use std::collections::HashMap;

use super::{desktop_mcp_secret_patches, desktop_mcp_url};
use loopal_config::{CwdIsolation, LayerSource, McpServerConfig, McpSharing};
use loopal_ipc::protocol::methods::{
    DesktopMcpCwdIsolation, DesktopMcpSecretStatus, DesktopMcpServerDefinition, DesktopMcpSharing,
};

pub(super) fn project(
    name: String,
    source: LayerSource,
    config: McpServerConfig,
) -> Option<DesktopMcpServerDefinition> {
    if !identifier(&name, 64) {
        return None;
    }
    let source = source_label(source);
    match config {
        McpServerConfig::Stdio {
            command,
            args,
            env,
            enabled,
            timeout_ms,
            sharing,
            cwd_isolation,
        } => {
            if !text(&command, 1024, false)
                || args.len() > 128
                || args.iter().any(|arg| !text(arg, 4096, true))
                || !timeout(timeout_ms)
                || cwd_isolation
                    .as_ref()
                    .is_some_and(|value| !valid_isolation(value))
            {
                return None;
            }
            Some(DesktopMcpServerDefinition::Stdio {
                name,
                source,
                command,
                args,
                enabled,
                timeout_ms,
                sharing: sharing_value(sharing),
                cwd_isolation: cwd_isolation.map(isolation_value),
                env: secret_statuses(env),
            })
        }
        McpServerConfig::StreamableHttp {
            url,
            headers,
            enabled,
            timeout_ms,
            sharing,
        } => {
            if desktop_mcp_secret_patches::validate_header_uniqueness(&headers).is_err() {
                return None;
            }
            let url = desktop_mcp_url::project(&url);
            if !text(&url, 2048, false) || !timeout(timeout_ms) {
                return None;
            }
            Some(DesktopMcpServerDefinition::StreamableHttp {
                name,
                source,
                url,
                enabled,
                timeout_ms,
                sharing: sharing_value(sharing),
                headers: secret_statuses(headers),
            })
        }
    }
}

fn secret_statuses(values: HashMap<String, String>) -> Vec<DesktopMcpSecretStatus> {
    let mut statuses: Vec<_> = values
        .into_iter()
        .filter(|(name, _)| text(name, 128, false))
        .map(|(name, value)| DesktopMcpSecretStatus {
            name,
            configured: !value.is_empty(),
        })
        .collect();
    statuses.sort_by(|a, b| a.name.cmp(&b.name));
    statuses
}

fn source_label(source: LayerSource) -> String {
    match source {
        LayerSource::Global => "global",
        LayerSource::Plugin(_) => "plugin",
        LayerSource::Project => "project",
        LayerSource::Local => "local",
        LayerSource::Env => "environment",
        LayerSource::Cli => "cli",
    }
    .into()
}

fn sharing_value(value: McpSharing) -> DesktopMcpSharing {
    match value {
        McpSharing::HubSingleton => DesktopMcpSharing::HubSingleton,
        McpSharing::PerAgent => DesktopMcpSharing::PerAgent,
        McpSharing::SpawnTree => DesktopMcpSharing::SpawnTree,
    }
}

fn isolation_value(value: CwdIsolation) -> DesktopMcpCwdIsolation {
    DesktopMcpCwdIsolation {
        arg: value.arg,
        cache_subdir: value.cache_subdir,
    }
}

fn valid_isolation(value: &CwdIsolation) -> bool {
    value.arg.starts_with('-')
        && text(&value.arg, 128, false)
        && value
            .cache_subdir
            .as_ref()
            .is_none_or(|value| identifier(value, 128))
}

fn timeout(value: u64) -> bool {
    (100..=600_000).contains(&value)
}

fn identifier(value: &str, max: usize) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && value.len() <= max
}

fn text(value: &str, max: usize, allow_empty: bool) -> bool {
    (allow_empty || !value.trim().is_empty())
        && value.len() <= max
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_duplicate_headers_do_not_break_the_typed_list() {
        let headers = HashMap::from([
            ("Authorization".into(), "first".into()),
            ("authorization".into(), "second".into()),
        ]);
        let server = McpServerConfig::StreamableHttp {
            url: "https://example.test/mcp".into(),
            headers,
            enabled: true,
            timeout_ms: 30_000,
            sharing: McpSharing::HubSingleton,
        };
        assert!(project("legacy".into(), LayerSource::Local, server).is_none());
    }
}
