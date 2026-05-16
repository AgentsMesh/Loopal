use std::io::Write;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_secret_runtime::{apply_redactor, apply_resolver};
use loopal_tool_api::ToolContext;
use loopal_vault_api::{SecretString, Vault, VaultResult};
use secrecy::ExposeSecret;
use serde_json::json;

#[derive(Default)]
struct MockVault {
    map: std::collections::HashMap<String, String>,
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

#[derive(Clone)]
struct SharedBufWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBufWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBufWriter {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn ctx_with_secret(name: &str, value: &str) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    let mut map = std::collections::HashMap::new();
    map.insert(name.to_string(), value.to_string());
    let store: Arc<dyn Vault> = Arc::new(MockVault { map });
    ToolContext::new(backend, "tracing-sentinel").with_secrets(store)
}

const SECRET_VALUE: &str = "sk-CANARY-1234567890-VERY-DISTINCT";

/// Sentinel test: under maximally verbose tracing, secret plaintext MUST NOT
/// appear in any captured log line during resolve + redact pipeline.
#[tokio::test]
async fn tracing_never_contains_plaintext_after_full_pipeline() {
    let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let writer = SharedBufWriter(buf.clone());

    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();

    let _guard = tracing::subscriber::set_default(subscriber);
    let ctx = ctx_with_secret("canary", SECRET_VALUE);

    let mut input = json!({
        "command": "curl -H 'Bearer <secret_ref:canary>'",
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
    assert_eq!(seed[0].1.expose_secret(), SECRET_VALUE);

    let simulated_output = format!("DEBUG: Authorization: Bearer {SECRET_VALUE}\nexit 0");
    let redacted = apply_redactor("Bash", simulated_output, &seed, "tracing-sentinel");
    assert!(!redacted.contains(SECRET_VALUE));

    drop(_guard);

    let captured = String::from_utf8_lossy(&buf.lock().unwrap()).to_string();
    assert!(
        !captured.contains(SECRET_VALUE),
        "tracing leaked plaintext: captured = {captured}"
    );
}
