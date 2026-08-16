use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;
use loopal_config::McpServerConfig;
use loopal_mcp::{HUB_RPC_BUDGET, LocalMcpProvider, McpManager, McpProvider};
use tokio::sync::RwLock;

fn make_provider() -> LocalMcpProvider {
    LocalMcpProvider::new(Arc::new(RwLock::new(McpManager::new())))
}

fn failing_config(name: &str, timeout_ms: u64) -> McpServerConfig {
    McpServerConfig::Stdio {
        command: name.into(),
        args: vec![],
        env: Default::default(),
        enabled: true,
        timeout_ms,
        sharing: Default::default(),
        cwd_isolation: None,
    }
}

include!("local_provider_test/settle_test.rs");
include!("local_provider_test/call_test.rs");
