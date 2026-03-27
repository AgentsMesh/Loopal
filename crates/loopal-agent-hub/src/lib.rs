//! AgentHub — central connection manager for multi-agent architecture.
//!
//! Manages root agent (stdio Bridge) and sub-agents (TCP) connections.
//! Acts as a network hub: all agent events fan-in here, then fan-out
//! to subscribed frontends (TUI, GUI, etc.).

mod connection_ops;
mod event_router;
mod hub;
mod types;

pub use event_router::start_event_loop;
pub use hub::AgentHub;
pub use types::PrimaryConn;
