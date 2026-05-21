use std::collections::HashSet;

use loopal_secret_client::SecretClient;
use loopal_secret_client::SecretError;
use loopal_secret_client::placeholder::{AUTHOR_RE, WIRE_RE};

#[derive(Debug, Clone, Default)]
pub struct TranslationView {
    pub known_names: HashSet<String>,
}

impl TranslationView {
    pub fn from_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            known_names: names.into_iter().map(Into::into).collect(),
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.known_names.contains(name)
    }
}

#[derive(Debug, Default)]
pub struct TranslationStats {
    pub translated: usize,
    pub missing: Vec<String>,
}

pub fn translate_outbound(
    input: &str,
    view: Option<&TranslationView>,
) -> (String, TranslationStats) {
    let mut stats = TranslationStats::default();
    let out = AUTHOR_RE
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            let known = view.map(|v| v.contains(name)).unwrap_or(false);
            if known {
                stats.translated += 1;
                format!("<secret_ref:{name}>")
            } else {
                stats.missing.push(name.to_string());
                format!("<missing-secret:{name}>")
            }
        })
        .into_owned();
    (out, stats)
}

pub fn collect_author_names(input: &str) -> Vec<String> {
    AUTHOR_RE
        .captures_iter(input)
        .map(|c| c[1].to_string())
        .collect()
}

pub fn collect_wire_names(input: &str) -> Vec<String> {
    WIRE_RE
        .captures_iter(input)
        .map(|c| c[1].to_string())
        .collect()
}

/// Async-expand `{{secret:NAME}}` placeholders to plaintext via the given client.
///
/// Unlike `translate_outbound` (which converts author syntax to wire syntax
/// for LLM-bound text), this resolves directly to plaintext. Use for fields
/// that must contain real secret values at point of use, e.g. provider
/// `api_key` / `base_url` and MCP server `env` / `headers` / `url`.
pub async fn expand_to_plaintext(input: &str, client: &dyn SecretClient) -> String {
    let names: Vec<String> = AUTHOR_RE
        .captures_iter(input)
        .map(|c| c[1].to_string())
        .collect();
    if names.is_empty() {
        return input.to_string();
    }
    let mut resolved: std::collections::HashMap<String, secrecy::SecretString> =
        std::collections::HashMap::new();
    for n in &names {
        match client.get(n).await {
            Ok(v) => {
                resolved.insert(n.clone(), v);
            }
            Err(SecretError::SecretNotFound(_)) => {}
            Err(e) => {
                tracing::warn!(
                    name = %n,
                    error = %e,
                    "expand_to_plaintext: secret fetch failed (non-NotFound); \
                     leaving as <missing-secret:NAME> placeholder"
                );
            }
        }
    }
    AUTHOR_RE
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            match resolved.get(name) {
                Some(v) => {
                    use secrecy::ExposeSecret;
                    v.expose_secret().to_string()
                }
                None => format!("<missing-secret:{name}>"),
            }
        })
        .into_owned()
}
