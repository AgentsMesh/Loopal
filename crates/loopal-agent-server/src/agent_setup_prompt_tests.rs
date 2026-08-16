use std::sync::Arc;

use loopal_vault_api::{SecretString, Vault, VaultResult};

use super::*;

struct NamedVault;

#[async_trait::async_trait]
impl Vault for NamedVault {
    async fn get(&self, _name: &str) -> Option<SecretString> {
        None
    }

    async fn list_names(&self) -> Vec<String> {
        vec!["service_token".into()]
    }

    async fn put(&self, _name: &str, _value: SecretString) -> VaultResult<()> {
        Ok(())
    }

    async fn delete(&self, _name: &str) -> VaultResult<()> {
        Ok(())
    }

    async fn rekey(&self) -> VaultResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn known_author_secrets_are_translated_before_prompt_assembly() {
    let mut config = loopal_config::ConfigResolver::new().resolve().unwrap();
    config.instructions = "token={{secret:service_token}}".into();
    config.secrets = Some(Arc::new(NamedVault));
    let kernel = loopal_kernel::Kernel::new(config.settings.clone()).unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let prompt = build_session_system_prompt(
        &config,
        &kernel,
        cwd.path(),
        SessionPromptOptions {
            mode: "act",
            agent_type: None,
            depth: 0,
            tool_defs: &[],
            features: Vec::new(),
        },
    )
    .await;

    assert!(prompt.contains("<secret_ref:service_token>"));
    assert!(!prompt.contains("{{secret:service_token}}"));
}
