use std::ffi::{OsStr, OsString};
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::cli::{ChildPassthroughArgs, Cli, ParentOnlyArgs};

pub struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    pub fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

pub fn assert_runtime_fixture() {
    for variable in ["LOOPAL_BINARY", "LOOPAL_TEST_PROVIDER"] {
        let path = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set"));
        assert!(std::path::Path::new(&path).is_file(), "missing {variable}");
    }
}

pub fn config(root: &std::path::Path, workflow: bool) -> loopal_config::ResolvedConfig {
    let mut settings = loopal_config::Settings {
        model: "bootstrap-coverage-model".into(),
        telemetry: loopal_config::TelemetryConfig {
            telemetry_dir: Some(root.join("telemetry").display().to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    if workflow {
        settings.workflow.execution_enabled = true;
        settings.workflow.policy = loopal_config::OrchestrationPolicy::Explicit;
    }
    loopal_config::ResolvedConfig {
        settings,
        workflow_preset_thinking_recommendation: None,
        mcp_servers: Default::default(),
        skills: Default::default(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        classifier_prompt: None,
        layers: Vec::new(),
        secrets: None,
    }
}

pub fn cli(parent_only: ParentOnlyArgs) -> Cli {
    Cli {
        child: ChildPassthroughArgs {
            permission: Some("yolo".into()),
            ..Default::default()
        },
        parent_only,
        prompt: Vec::new(),
    }
}

pub async fn wait_for_record(pid: u32) -> super::discovery::HubDiscoveryRecord {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut interval = tokio::time::interval(Duration::from_millis(5));
        loop {
            interval.tick().await;
            if let Ok(record) = super::discovery::read_record(pid) {
                return record;
            }
        }
    })
    .await
    .expect("Hub discovery record deadline")
}

pub async fn register_ui(
    addr: &str,
    token: &str,
) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
    let stream = TcpStream::connect(addr)
        .await
        .expect("connect bootstrap Hub");
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (connection, incoming) = Connection::new(transport).into_listening();
    let response = connection
        .send_request(
            methods::HUB_REGISTER.name,
            serde_json::json!({
                "name": "bootstrap-coverage-ui",
                "token": token,
                "role": "ui_client",
                "capabilities": {
                    "permission": true,
                    "question": true,
                    "plan_approval": true,
                },
            }),
        )
        .await
        .expect("register bootstrap UI");
    assert_eq!(response["ok"], true);
    (connection, incoming)
}
