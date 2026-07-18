//! Full-stack Rust CLI behavior tests: real `loopal --serve` agent process ×
//! in-process mock LLM HTTP server, over the production provider adapter/wire.

#[path = "support.rs"]
pub mod support;

#[path = "basic_test.rs"]
mod basic_test;

#[path = "tool_loop_test.rs"]
mod tool_loop_test;

#[path = "resilience_test.rs"]
mod resilience_test;
