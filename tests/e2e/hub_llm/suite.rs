//! Full-topology behavior tests: real `loopal --hub-only` (Hub + root Agent
//! processes, Hub-owned MCP subprocesses) × in-process mock LLM HTTP server,
//! driven over the Hub's TCP attach protocol.

#[path = "support.rs"]
pub mod support;

#[path = "mcp_via_hub_test.rs"]
mod mcp_via_hub_test;
