//! AgentHub — central connection manager and message router.
//!
//! Manages all agent connections (root + sub-agents + observers).
//! Provides message routing, pub/sub channels, and agent lifecycle management.
//! Designed as a standalone service — all agents connect via TCP.

pub mod agent_io;
mod connection_ops;
pub mod dispatch;
mod event_router;
mod hub;
pub mod hub_server;
mod routing;
pub mod spawn_manager;
pub mod topology;
mod tui_relay;
mod types;

pub use event_router::start_event_loop;
pub use hub::AgentHub;
pub use topology::{AgentInfo, AgentLifecycle};
pub use types::LocalChannels;
