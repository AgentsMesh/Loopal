use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_mock_llm_server::MockLlmServer;
use serde_json::Value;
use tokio::sync::mpsc::Receiver;

use super::hub_controls::{HarnessVault, PermissionDesk};
use super::process::{
    API_KEY, Provider, boot_process, write_mcp_settings, write_telemetry_settings,
};

pub struct CliHarness {
    pub base_url: String,
    pub(super) conn: Arc<Connection<Listening>>,
    pub(super) rx: Receiver<Incoming>,
    child: tokio::process::Child,
    mock: MockLlmServer,
    cwd: tempfile::TempDir,
    home: tempfile::TempDir,
    pub(super) provider: Provider,
    mcp: bool,
    mcp_calls: Arc<Mutex<Vec<Value>>>,
    vault: Arc<HarnessVault>,
    permissions: Arc<PermissionDesk>,
}

impl CliHarness {
    pub async fn start(scenario: Value) -> Self {
        Self::spawn(scenario, Provider::Anthropic, false, false).await
    }

    pub async fn start_with_telemetry(scenario: Value) -> Self {
        Self::spawn(scenario, Provider::Anthropic, false, true).await
    }

    pub async fn start_with(scenario: Value, provider: Provider) -> Self {
        Self::spawn(scenario, provider, false, false).await
    }

    pub async fn start_with_mcp(scenario: Value) -> Self {
        Self::spawn(scenario, Provider::Anthropic, true, false).await
    }

    async fn spawn(
        scenario: Value,
        provider: Provider,
        mcp: bool,
        deterministic_telemetry: bool,
    ) -> Self {
        let mock = MockLlmServer::start(scenario, API_KEY).await;
        let base_url = mock.base_url().to_owned();
        let home = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        if deterministic_telemetry {
            write_telemetry_settings(home.path());
        }
        if mcp {
            write_mcp_settings(cwd.path());
        }
        let mcp_calls = Arc::new(Mutex::new(Vec::new()));
        let vault = Arc::new(HarnessVault::default());
        let permissions = Arc::new(PermissionDesk::default());
        let (child, conn, rx) = boot_process(
            provider,
            home.path(),
            &base_url,
            mcp,
            mcp_calls.clone(),
            vault.clone(),
            permissions.clone(),
        )
        .await;

        Self {
            base_url,
            conn,
            rx,
            child,
            mock,
            cwd,
            home,
            provider,
            mcp,
            mcp_calls,
            vault,
            permissions,
        }
    }

    pub async fn restart(&mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
        let (child, conn, rx) = boot_process(
            self.provider,
            self.home.path(),
            &self.base_url,
            self.mcp,
            self.mcp_calls.clone(),
            self.vault.clone(),
            self.permissions.clone(),
        )
        .await;
        self.child = child;
        self.conn = conn;
        self.rx = rx;
    }

    pub fn permissions(&self) -> Arc<PermissionDesk> {
        self.permissions.clone()
    }

    pub fn cwd(&self) -> &Path {
        self.cwd.path()
    }

    pub fn telemetry_dir(&self) -> PathBuf {
        self.home.path().join(".loopal").join("telemetry")
    }

    pub fn mcp_calls(&self) -> Vec<Value> {
        self.mcp_calls.lock().unwrap().clone()
    }

    pub fn vault(&self) -> Arc<HarnessVault> {
        self.vault.clone()
    }

    pub async fn journal(&self) -> Value {
        self.mock.requests().await
    }

    pub async fn verify(&self) -> Value {
        self.mock.verify().await
    }
}
