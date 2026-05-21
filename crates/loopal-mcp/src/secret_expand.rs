use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::McpServerConfig;
use loopal_secret_client::SecretClient;
use loopal_secret_runtime::expand_to_plaintext;

pub async fn expand_mcp_config(
    config: McpServerConfig,
    store: Option<&Arc<dyn SecretClient>>,
) -> McpServerConfig {
    let Some(store) = store else {
        warn_if_placeholders_unresolved(&config);
        return config;
    };
    let v = store.as_ref();
    match config {
        McpServerConfig::Stdio {
            command,
            args,
            env,
            enabled,
            timeout_ms,
            sharing,
            cwd_isolation,
        } => McpServerConfig::Stdio {
            command: expand_to_plaintext(&command, v).await,
            args: expand_vec(args, v).await,
            env: expand_map(env, v).await,
            enabled,
            timeout_ms,
            sharing,
            cwd_isolation,
        },
        McpServerConfig::StreamableHttp {
            url,
            headers,
            enabled,
            timeout_ms,
            sharing,
        } => McpServerConfig::StreamableHttp {
            url: expand_to_plaintext(&url, v).await,
            headers: expand_map(headers, v).await,
            enabled,
            timeout_ms,
            sharing,
        },
    }
}

async fn expand_vec(items: Vec<String>, v: &dyn SecretClient) -> Vec<String> {
    let mut out = Vec::with_capacity(items.len());
    for s in items {
        out.push(expand_to_plaintext(&s, v).await);
    }
    out
}

async fn expand_map(
    map: HashMap<String, String>,
    v: &dyn SecretClient,
) -> HashMap<String, String> {
    let mut out = HashMap::with_capacity(map.len());
    for (k, val) in map {
        out.insert(k, expand_to_plaintext(&val, v).await);
    }
    out
}

/// Diagnostic: when no vault is configured but the MCP server config still
/// contains `{{secret:...}}` placeholders, the server will receive the
/// literal placeholder string. That's almost always a misconfiguration —
/// warn the user once so the failure isn't silent.
fn warn_if_placeholders_unresolved(config: &McpServerConfig) {
    let placeholder = "{{secret:";
    let (name, unresolved) = match config {
        McpServerConfig::Stdio {
            command, args, env, ..
        } => {
            let in_command = command.contains(placeholder);
            let in_args = args.iter().any(|a| a.contains(placeholder));
            let in_env = env.values().any(|v| v.contains(placeholder));
            ("stdio", in_command || in_args || in_env)
        }
        McpServerConfig::StreamableHttp { url, headers, .. } => {
            let in_url = url.contains(placeholder);
            let in_headers = headers.values().any(|v| v.contains(placeholder));
            ("streamable-http", in_url || in_headers)
        }
    };
    if unresolved {
        tracing::warn!(
            transport = name,
            "MCP server config references {{secret:...}} placeholders but no \
             vault is configured — the server will receive the literal \
             placeholder string. Configure a vault or remove the placeholders."
        );
    }
}
