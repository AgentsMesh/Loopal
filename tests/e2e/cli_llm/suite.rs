//! Full-stack Rust CLI behavior tests: real `loopal --serve` agent process ×
//! in-process mock LLM HTTP server, over the production provider adapter/wire.

#[path = "support.rs"]
pub mod support;

#[path = "basic_test.rs"]
mod basic_test;

#[path = "multi_provider_test.rs"]
mod multi_provider_test;

#[path = "tool_loop_test.rs"]
mod tool_loop_test;

#[path = "resilience_test.rs"]
mod resilience_test;

#[path = "cancel_test.rs"]
mod cancel_test;

#[path = "behaviors_test.rs"]
mod behaviors_test;

#[path = "multiturn_test.rs"]
mod multiturn_test;

#[path = "compaction_test.rs"]
mod compaction_test;

#[path = "mcp_test.rs"]
mod mcp_test;

#[path = "secrets_test.rs"]
mod secrets_test;

#[path = "degraded_test.rs"]
mod degraded_test;

#[path = "permission_test.rs"]
mod permission_test;

#[path = "resume_test.rs"]
mod resume_test;
