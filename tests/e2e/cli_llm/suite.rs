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
