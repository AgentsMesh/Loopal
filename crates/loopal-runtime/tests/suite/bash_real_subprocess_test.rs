use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use loopal_secret_runtime::{apply_redactor, apply_resolver};
use loopal_tool_api::backend_types::EnvOverride;
use loopal_tool_api::{Backend, ToolContext};
use loopal_vault_api::{SecretString, Vault, VaultResult};
use secrecy::ExposeSecret;
use serde_json::json;

const CANARY: &str = "sk-REAL-PROC-CANARY-9876543210";

struct MockStore(HashMap<String, String>);

#[async_trait]
impl Vault for MockStore {
    async fn get(&self, name: &str) -> Option<SecretString> {
        self.0.get(name).map(|v| SecretString::from(v.clone()))
    }
    async fn list_names(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
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

fn build_ctx(store: Arc<dyn Vault>) -> (Arc<dyn Backend>, ToolContext) {
    let backend: Arc<dyn Backend> = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
        "bash-e2e",
    );
    let ctx = ToolContext::new(backend.clone(), "bash-e2e").with_secrets(store);
    (backend, ctx)
}

#[cfg(unix)]
#[tokio::test]
async fn end_to_end_bash_env_injects_plaintext_and_redacts_output() {
    let mut map = HashMap::new();
    map.insert("canary".to_string(), CANARY.to_string());
    let store: Arc<dyn Vault> = Arc::new(MockStore(map));
    let (backend, ctx) = build_ctx(store);

    // Simulated LLM tool_use: secret in `env` (preferred), echoed by command.
    let mut input = json!({
        "command": "echo $TOKEN",
        "env": { "TOKEN": "<secret_ref:canary>" }
    });

    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command", "env"],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;
    assert_eq!(seed.len(), 1);
    assert_eq!(seed[0].1.expose_secret(), CANARY);

    // Real child process: env contains plaintext, command echoes it.
    let env = EnvOverride::new().with("TOKEN", input["env"]["TOKEN"].as_str().unwrap().to_string());
    let cmd = input["command"].as_str().unwrap();
    let result = backend
        .exec(cmd, std::time::Duration::from_secs(5), &env)
        .await
        .expect("real subprocess should succeed");

    assert!(
        result.stdout.contains(CANARY),
        "real shell echoed plaintext from env (proves real subprocess pathway): {}",
        result.stdout
    );

    // Now redact: the plaintext that briefly existed in stdout must be scrubbed.
    let redacted = apply_redactor("Bash", result.stdout, &seed, "bash-e2e");
    assert!(
        !redacted.contains(CANARY),
        "redactor MUST scrub plaintext from tool output before it reaches LLM"
    );
    assert!(
        redacted.contains("<secret_ref:canary>"),
        "redactor MUST substitute placeholder back: {redacted}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn end_to_end_command_substitution_warns_but_still_works() {
    let mut map = HashMap::new();
    map.insert("canary".to_string(), CANARY.to_string());
    let store: Arc<dyn Vault> = Arc::new(MockStore(map));
    let (backend, ctx) = build_ctx(store);

    // LLM puts secret IN the command string (less safe, visible to `ps`).
    let mut input = json!({
        "command": "echo plaintext=<secret_ref:canary>"
    });
    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command", "env"],
        ctx.secrets.as_ref(),
        &ctx.session_id,
    )
    .await;
    assert_eq!(seed.len(), 1);

    let cmd = input["command"].as_str().unwrap();
    assert!(cmd.contains(CANARY), "resolver substituted into command");

    let result = backend
        .exec(
            cmd,
            std::time::Duration::from_secs(5),
            &EnvOverride::default(),
        )
        .await
        .expect("subprocess");
    assert!(result.stdout.contains(CANARY));

    // Redactor still cleans output regardless of injection style.
    let redacted = apply_redactor("Bash", result.stdout, &seed, "bash-e2e");
    assert!(!redacted.contains(CANARY));
    assert!(redacted.contains("<secret_ref:canary>"));
}
