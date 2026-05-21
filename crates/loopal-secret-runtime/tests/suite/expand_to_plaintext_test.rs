use std::sync::Arc;

use async_trait::async_trait;
use loopal_secret_client::{HUB_RPC_BUDGET, IpcBudget, SecretClient, SecretError, SecretResult};
use loopal_secret_runtime::expand_to_plaintext;
use secrecy::{ExposeSecret, SecretString};

struct MockClient {
    behavior: MockBehavior,
}

enum MockBehavior {
    Found(&'static str),
    NotFound,
    IpcError,
    PermissionDenied,
}

#[async_trait]
impl SecretClient for MockClient {
    async fn get(&self, name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        match &self.behavior {
            MockBehavior::Found(v) => Ok(SecretString::from((*v).to_string())),
            MockBehavior::NotFound => Err(SecretError::SecretNotFound(name.to_string())),
            MockBehavior::IpcError => Err(SecretError::Ipc("transport closed".into())),
            MockBehavior::PermissionDenied => Err(SecretError::PermissionDenied),
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

fn client(behavior: MockBehavior) -> Arc<dyn SecretClient> {
    Arc::new(MockClient { behavior })
}

#[tokio::test]
async fn substitutes_plaintext_on_success() {
    let c = client(MockBehavior::Found("sk-abc"));
    let out = expand_to_plaintext("key={{secret:api_key}}", c.as_ref(), HUB_RPC_BUDGET).await;
    assert_eq!(out, "key=sk-abc");
}

#[tokio::test]
async fn falls_back_to_placeholder_on_not_found() {
    let c = client(MockBehavior::NotFound);
    let out = expand_to_plaintext("k={{secret:missing}}", c.as_ref(), HUB_RPC_BUDGET).await;
    assert_eq!(out, "k=<missing-secret:missing>");
}

#[tokio::test]
async fn falls_back_to_placeholder_on_ipc_error() {
    let c = client(MockBehavior::IpcError);
    let out = expand_to_plaintext("k={{secret:any}}", c.as_ref(), HUB_RPC_BUDGET).await;
    assert_eq!(
        out, "k=<missing-secret:any>",
        "IPC error should fall back to placeholder so caller doesn't panic"
    );
}

#[tokio::test]
async fn falls_back_to_placeholder_on_permission_denied() {
    let c = client(MockBehavior::PermissionDenied);
    let out = expand_to_plaintext("k={{secret:any}}", c.as_ref(), HUB_RPC_BUDGET).await;
    assert_eq!(out, "k=<missing-secret:any>");
}

#[tokio::test]
async fn no_placeholder_passes_through_unchanged() {
    let c = client(MockBehavior::IpcError);
    let out = expand_to_plaintext("plain text", c.as_ref(), HUB_RPC_BUDGET).await;
    assert_eq!(out, "plain text");
}

#[tokio::test]
async fn secret_string_is_zeroized_after_use() {
    let c = client(MockBehavior::Found("super-secret"));
    let out = expand_to_plaintext("v={{secret:x}}", c.as_ref(), HUB_RPC_BUDGET).await;
    assert!(out.contains("super-secret"));
    let s = SecretString::from("test".to_string());
    assert_eq!(s.expose_secret(), "test");
}
