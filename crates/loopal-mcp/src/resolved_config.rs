use std::collections::HashMap;

use loopal_config::McpServerConfig;
use loopal_secret_client::{ExposeSecret, SecretString};
use zeroize::Zeroizing;

pub(crate) enum ResolvedMcpConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, Zeroizing<String>>,
    },
    StreamableHttp {
        headers: HashMap<String, Zeroizing<String>>,
    },
}

impl ResolvedMcpConfig {
    pub(crate) fn from_config(config: McpServerConfig, seed: &[(String, SecretString)]) -> Self {
        let values: HashMap<_, _> = seed
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect();
        match config {
            McpServerConfig::Stdio {
                command, args, env, ..
            } => Self::Stdio {
                command,
                args,
                env: expand_map(env, &values),
            },
            McpServerConfig::StreamableHttp { headers, .. } => Self::StreamableHttp {
                headers: expand_map(headers, &values),
            },
        }
    }
}

fn expand_map(
    map: HashMap<String, String>,
    values: &HashMap<&str, &SecretString>,
) -> HashMap<String, Zeroizing<String>> {
    map.into_iter()
        .map(|(key, value)| (key, Zeroizing::new(expand_value(&value, values))))
        .collect()
}

fn expand_value(input: &str, values: &HashMap<&str, &SecretString>) -> String {
    loopal_secret_client::AUTHOR_RE
        .replace_all(input, |captures: &regex::Captures<'_>| {
            values[&captures[1]].expose_secret()
        })
        .into_owned()
}
