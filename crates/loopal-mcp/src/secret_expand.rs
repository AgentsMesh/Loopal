use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::McpServerConfig;
use loopal_secret_runtime::expand_to_plaintext;
use loopal_vault_api::Vault;

pub async fn expand_mcp_config(
    config: McpServerConfig,
    store: Option<&Arc<dyn Vault>>,
) -> McpServerConfig {
    let Some(store) = store else {
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
        } => McpServerConfig::Stdio {
            command: expand_to_plaintext(&command, v).await,
            args: expand_vec(args, v).await,
            env: expand_map(env, v).await,
            enabled,
            timeout_ms,
        },
        McpServerConfig::StreamableHttp {
            url,
            headers,
            enabled,
            timeout_ms,
        } => McpServerConfig::StreamableHttp {
            url: expand_to_plaintext(&url, v).await,
            headers: expand_map(headers, v).await,
            enabled,
            timeout_ms,
        },
    }
}

async fn expand_vec(items: Vec<String>, v: &dyn Vault) -> Vec<String> {
    let mut out = Vec::with_capacity(items.len());
    for s in items {
        out.push(expand_to_plaintext(&s, v).await);
    }
    out
}

async fn expand_map(map: HashMap<String, String>, v: &dyn Vault) -> HashMap<String, String> {
    let mut out = HashMap::with_capacity(map.len());
    for (k, val) in map {
        out.insert(k, expand_to_plaintext(&val, v).await);
    }
    out
}
