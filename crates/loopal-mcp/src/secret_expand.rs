use std::sync::Arc;

use loopal_config::McpServerConfig;
use loopal_ipc::IpcBudget;
use loopal_secret_client::{AUTHOR_RE, SecretClient, SecretString, WIRE_RE, collect_names};

use crate::resolved_config::ResolvedMcpConfig;
use crate::secret_provenance::SecretProvenance;

pub const CONFIG_SECRET_ERROR: &str = "MCP server secret configuration unavailable";

pub(crate) async fn expand_mcp_config(
    config: McpServerConfig,
    client: Option<&Arc<dyn SecretClient>>,
    provenance: &SecretProvenance,
    budget: IpcBudget,
) -> Result<ResolvedMcpConfig, &'static str> {
    let seed = resolve_bound_mcp_secret_seed(&config, client, provenance, budget).await?;
    Ok(ResolvedMcpConfig::from_config(config, &seed))
}

pub(crate) async fn resolve_bound_mcp_secret_seed(
    config: &McpServerConfig,
    client: Option<&Arc<dyn SecretClient>>,
    provenance: &SecretProvenance,
    budget: IpcBudget,
) -> Result<Vec<(String, SecretString)>, &'static str> {
    let seed = resolve_mcp_secret_seed(config, client, budget).await?;
    provenance.establish(&seed)?;
    Ok(seed)
}

pub(crate) async fn resolve_mcp_secret_seed(
    config: &McpServerConfig,
    client: Option<&Arc<dyn SecretClient>>,
    budget: IpcBudget,
) -> Result<Vec<(String, SecretString)>, &'static str> {
    reject_ineligible_fields(config)?;
    reject_wire_placeholders(config)?;
    let names = eligible_names(config);
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let client = client.ok_or(CONFIG_SECRET_ERROR)?;
    fetch_seed(names, client.as_ref(), budget).await
}

fn reject_ineligible_fields(config: &McpServerConfig) -> Result<(), &'static str> {
    let invalid = match config {
        McpServerConfig::Stdio {
            command,
            args,
            env,
            cwd_isolation,
            ..
        } => {
            contains_placeholder(command)
                || args.iter().any(|value| contains_placeholder(value))
                || env.keys().any(|key| contains_placeholder(key))
                || cwd_isolation.as_ref().is_some_and(|isolation| {
                    contains_placeholder(&isolation.arg)
                        || isolation
                            .cache_subdir
                            .as_deref()
                            .is_some_and(contains_placeholder)
                })
        }
        McpServerConfig::StreamableHttp { url, headers, .. } => {
            contains_placeholder(url) || headers.keys().any(|key| contains_placeholder(key))
        }
    };
    (!invalid).then_some(()).ok_or(CONFIG_SECRET_ERROR)
}

fn reject_wire_placeholders(config: &McpServerConfig) -> Result<(), &'static str> {
    let invalid = match config {
        McpServerConfig::Stdio { env, .. } => env.values().any(|value| WIRE_RE.is_match(value)),
        McpServerConfig::StreamableHttp { headers, .. } => {
            headers.values().any(|value| WIRE_RE.is_match(value))
        }
    };
    (!invalid).then_some(()).ok_or(CONFIG_SECRET_ERROR)
}

fn eligible_names(config: &McpServerConfig) -> Vec<String> {
    let values: Box<dyn Iterator<Item = &String> + '_> = match config {
        McpServerConfig::Stdio { env, .. } => Box::new(env.values()),
        McpServerConfig::StreamableHttp { headers, .. } => Box::new(headers.values()),
    };
    let mut names = Vec::new();
    for value in values {
        names.extend(collect_names(&AUTHOR_RE, value));
    }
    names.sort();
    names.dedup();
    names
}

async fn fetch_seed(
    names: Vec<String>,
    client: &dyn SecretClient,
    budget: IpcBudget,
) -> Result<Vec<(String, SecretString)>, &'static str> {
    let mut seed = Vec::with_capacity(names.len());
    for name in names {
        let value = client
            .get(&name, budget)
            .await
            .map_err(|_| CONFIG_SECRET_ERROR)?;
        seed.push((name, value));
    }
    Ok(seed)
}

fn contains_placeholder(value: &str) -> bool {
    AUTHOR_RE.is_match(value) || WIRE_RE.is_match(value)
}
