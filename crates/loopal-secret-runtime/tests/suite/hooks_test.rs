use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use loopal_secret_client::{IpcBudget, SecretClient, SecretError, SecretResult};
use loopal_secret_runtime::{
    apply_redactor, apply_resolver, detect_argv_exposure, record_redaction_hits,
};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

struct MockClient {
    calls: AtomicUsize,
}

#[async_trait]
impl SecretClient for MockClient {
    async fn get(&self, name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        match name {
            "present" => Ok(SecretString::from("sk-present")),
            "env_only" => Ok(SecretString::from("env-secret")),
            "ghost" => Err(SecretError::SecretNotFound(name.into())),
            _ => Err(SecretError::PermissionDenied),
        }
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn expand_author(
        &self,
        template: &str,
        _budget: IpcBudget,
    ) -> SecretResult<SecretString> {
        Ok(SecretString::from(template.to_string()))
    }

    async fn expand_wire(&self, template: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        Ok(SecretString::from(template.to_string()))
    }
}

fn client() -> Arc<MockClient> {
    Arc::new(MockClient {
        calls: AtomicUsize::new(0),
    })
}

struct HomeGuard(Option<std::ffi::OsString>);

impl HomeGuard {
    fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", path) };
        Self(previous)
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
    }
}

#[tokio::test]
async fn resolver_early_returns_preserve_placeholders() {
    let mut input = json!({"command": "<secret_ref:present>"});
    assert!(
        apply_resolver("Bash", &mut input, &["command"], None, "session")
            .await
            .is_empty()
    );

    let client = client();
    let secret_client: Arc<dyn SecretClient> = client.clone();
    assert!(
        apply_resolver("Bash", &mut input, &[], Some(&secret_client), "session")
            .await
            .is_empty()
    );
    let mut no_refs = json!({
        "command": "echo safe",
        "description": "<secret_ref:present>"
    });
    assert!(
        apply_resolver(
            "Bash",
            &mut no_refs,
            &["command"],
            Some(&secret_client),
            "session",
        )
        .await
        .is_empty()
    );
    assert_eq!(client.calls.load(Ordering::Relaxed), 0);
    assert_eq!(input["command"], "<secret_ref:present>");
    assert_eq!(no_refs["description"], "<secret_ref:present>");
}

#[tokio::test]
async fn resolver_and_redactor_keep_plaintext_consumer_scoped() {
    let home = tempfile::tempdir().unwrap();
    let _home = HomeGuard::set(home.path());
    let client = client();
    let secret_client: Arc<dyn SecretClient> = client;
    let mut input = json!({
        "command": "run <secret_ref:present> <secret_ref:ghost>",
        "env": {"TOKEN": "<secret_ref:env_only>"},
        "description": "<secret_ref:present>"
    });

    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command", "env"],
        Some(&secret_client),
        "session-1",
    )
    .await;

    assert_eq!(input["command"], "run sk-present <missing-secret:ghost>");
    assert_eq!(input["env"]["TOKEN"], "env-secret");
    assert_eq!(input["description"], "<secret_ref:present>");
    assert_eq!(detect_argv_exposure(&input, &seed), ["present"]);
    assert_eq!(seed.len(), 2);
    assert!(
        seed.iter()
            .any(|(name, value)| { name == "env_only" && value.expose_secret() == "env-secret" })
    );

    assert_eq!(
        apply_redactor("Bash", "leaked sk-present".into(), &seed, "session-1"),
        "leaked <secret_ref:present>"
    );
    assert_eq!(
        apply_redactor("Bash", "safe".into(), &[], "session-1"),
        "safe"
    );
    record_redaction_hits("Bash", &[], "session-1");

    let audit =
        std::fs::read_to_string(home.path().join(".loopal/telemetry/secret_access.jsonl")).unwrap();
    assert!(audit.contains("present"));
    assert!(audit.contains("env_only"));
    assert!(audit.contains("session-1"));
    assert!(!audit.contains("sk-present"));
    assert!(!audit.contains("env-secret"));

    let telemetry_dir = home.path().join(".loopal/telemetry");
    std::fs::remove_dir_all(&telemetry_dir).unwrap();
    std::fs::write(&telemetry_dir, "not a directory").unwrap();
    record_redaction_hits("Bash", &[String::from("present")], "session-audit-error");
}

#[tokio::test]
async fn resolver_hard_failure_preserves_all_wire_refs() {
    let client: Arc<dyn SecretClient> = client();
    let mut input = json!({
        "command": "run <secret_ref:present> <secret_ref:bad>",
        "env": {"TOKEN": "<secret_ref:env_only>"}
    });
    let original = input.clone();

    let seed = apply_resolver(
        "Bash",
        &mut input,
        &["command", "env"],
        Some(&client),
        "session-hard-failure",
    )
    .await;

    assert!(seed.is_empty());
    assert_eq!(input, original);
}
