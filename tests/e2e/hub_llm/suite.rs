//! Full-topology behavior tests: real `loopal --hub-only` (Hub + root Agent
//! processes, Hub-owned MCP subprocesses) × in-process mock LLM HTTP server,
//! driven over the Hub's TCP attach protocol.

#![cfg(unix)]

#[path = "support.rs"]
pub mod support;

#[path = "mcp_reconnect_test.rs"]
mod mcp_reconnect_test;

#[path = "mcp_via_hub_test.rs"]
mod mcp_via_hub_test;

#[path = "subagent_test.rs"]
mod subagent_test;

#[path = "observer_test.rs"]
mod observer_test;

#[path = "protected_audit_test.rs"]
mod protected_audit_test;

#[path = "vault_test.rs"]
mod vault_test;

#[path = "workflow_test.rs"]
mod workflow_test;

#[path = "workflow_retry_test.rs"]
mod workflow_retry_test;

#[path = "workflow_deadline_test.rs"]
mod workflow_deadline_test;

#[path = "workflow_secret_permission_test.rs"]
mod workflow_secret_permission_test;

#[path = "workflow_stale_completion_test.rs"]
mod workflow_stale_completion_test;

#[path = "workflow_cancel_test.rs"]
mod workflow_cancel_test;

#[path = "workflow_recovery_test.rs"]
mod workflow_recovery_test;

#[path = "workflow_crash_recovery_test.rs"]
mod workflow_crash_recovery_test;

#[path = "workflow_terminal_delivery_test.rs"]
mod workflow_terminal_delivery_test;

#[path = "workflow_terminal_crash_delivery_test.rs"]
mod workflow_terminal_crash_delivery_test;

#[path = "workflow_terminal_duplicate_test.rs"]
mod workflow_terminal_duplicate_test;
