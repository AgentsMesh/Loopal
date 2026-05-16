use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_secret_runtime::{apply_redactor, apply_resolver};
use loopal_tool_api::ToolContext;
use loopal_vault_api::{SecretString, Vault, VaultResult};
use secrecy::ExposeSecret;
use serde_json::json;

#[derive(Default)]
struct MockVault {
    map: HashMap<String, String>,
}

impl MockVault {
    fn new(pairs: &[(&str, &str)]) -> Self {
        Self {
            map: pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }
}

#[async_trait]
impl Vault for MockVault {
    async fn get(&self, name: &str) -> Option<SecretString> {
        self.map.get(name).map(|v| SecretString::from(v.clone()))
    }
    async fn list_names(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }
    async fn put(&self, _: &str, _: SecretString) -> VaultResult<()> {
        Ok(())
    }
    async fn delete(&self, _: &str) -> VaultResult<()> {
        Ok(())
    }
    async fn rekey(&self) -> VaultResult<()> {
        Ok(())
    }
}

fn ctx_with_secrets(store: Arc<dyn Vault>) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test-session").with_secrets(store)
}

fn ctx_without_secrets() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test-session")
}

#[tokio::test]
async fn resolver_substitutes_whitelisted_field_only() {
    let store = Arc::new(MockVault::new(&[("openai_key", "sk-abc12345")]));
    let ctx = ctx_with_secrets(store);
    let mut input = json!({
        "command": "curl -H 'Bearer <secret_ref:openai_key>'",
        "description": "ignore <secret_ref:openai_key>"
    });
    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command"],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;

    assert_eq!(input["command"], json!("curl -H 'Bearer sk-abc12345'"));
    assert_eq!(
        input["description"],
        json!("ignore <secret_ref:openai_key>")
    );
    assert_eq!(seed.len(), 1);
    assert_eq!(seed[0].0, "openai_key");
    assert_eq!(seed[0].1.expose_secret(), "sk-abc12345");
}

#[tokio::test]
async fn resolver_no_op_when_no_store() {
    let ctx = ctx_without_secrets();
    let mut input = json!({ "command": "echo <secret_ref:foo>" });
    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command"],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;
    assert!(seed.is_empty());
    assert_eq!(input["command"], json!("echo <secret_ref:foo>"));
}

#[tokio::test]
async fn resolver_no_op_when_whitelist_empty() {
    let store = Arc::new(MockVault::new(&[("k", "12345678")]));
    let ctx = ctx_with_secrets(store);
    let mut input = json!({ "command": "<secret_ref:k>" });
    let seed = apply_resolver(
        "Write",
        &mut input,
        &[],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;
    assert!(seed.is_empty());
    assert_eq!(input["command"], json!("<secret_ref:k>"));
}

#[tokio::test]
async fn resolver_missing_secret_becomes_placeholder() {
    let store = Arc::new(MockVault::new(&[]));
    let ctx = ctx_with_secrets(store);
    let mut input = json!({ "command": "<secret_ref:ghost>" });
    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command"],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;
    assert!(seed.is_empty());
    assert_eq!(input["command"], json!("<missing-secret:ghost>"));
}

#[test]
fn redactor_scrubs_known_plaintext() {
    let seed = vec![("openai_key".to_string(), SecretString::from("sk-abc12345"))];
    let output = apply_redactor(
        "Bash",
        "curl returned: 401 with token sk-abc12345 on line 5".to_string(),
        &seed,
        "session-1",
    );
    assert_eq!(
        output,
        "curl returned: 401 with token <secret_ref:openai_key> on line 5"
    );
}

#[test]
fn redactor_no_op_when_no_seed() {
    let seed: Vec<(String, SecretString)> = Vec::new();
    let raw = "plain output with no secrets".to_string();
    let output = apply_redactor("Bash", raw.clone(), &seed, "session-1");
    assert_eq!(output, raw);
}

#[test]
fn redactor_handles_multiple_secrets() {
    let seed = vec![
        ("a".to_string(), SecretString::from("secret_aaa")),
        ("b".to_string(), SecretString::from("secret_bbb")),
    ];
    let output = apply_redactor(
        "Bash",
        "first=secret_aaa second=secret_bbb both=secret_aaa".to_string(),
        &seed,
        "session-1",
    );
    assert_eq!(
        output,
        "first=<secret_ref:a> second=<secret_ref:b> both=<secret_ref:a>"
    );
}

#[tokio::test]
async fn end_to_end_resolve_execute_redact_chain() {
    let store = Arc::new(MockVault::new(&[("api_key", "sk-tokenvalue")]));
    let ctx = ctx_with_secrets(store);

    let mut tool_args = json!({
        "command": "echo Bearer <secret_ref:api_key>"
    });
    let seed = apply_resolver(
        "Bash",
        &mut tool_args,
        &["command"],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;

    let resolved_cmd = tool_args["command"].as_str().unwrap().to_string();
    assert_eq!(resolved_cmd, "echo Bearer sk-tokenvalue");

    let simulated_output = "Bearer sk-tokenvalue\nexit 0".to_string();
    let redacted = apply_redactor("Bash", simulated_output, &seed, "session-1");
    assert_eq!(redacted, "Bearer <secret_ref:api_key>\nexit 0");

    assert!(!redacted.contains("sk-tokenvalue"));
}
