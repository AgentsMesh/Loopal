//! Hub — central coordinator for agents and UI clients.
//!
//! Split into specialized subsystems:
//! - `AgentRegistry` — agent connections, lifecycle, routing
//! - `UiDispatcher` — UI client connections, event broadcast
//! - `UiSession` — client-side handle for UI clients
//! - `HubClient` — typed Hub communication interface

pub mod agent_io;
pub mod agent_registry;
pub mod dispatch;
mod event_router;
mod hub;
pub mod hub_server;
mod hub_ui_client;
mod routing;
pub mod spawn_manager;
pub mod topology;
mod types;
mod ui_dispatcher;
mod ui_relay;
pub mod ui_session;

pub use agent_registry::AgentRegistry;
pub use event_router::start_event_loop;
pub use hub::Hub;
pub use hub_ui_client::HubClient;
pub use topology::{AgentInfo, AgentLifecycle};
pub use types::LocalChannels;
pub use ui_dispatcher::UiDispatcher;
pub use ui_session::UiSession;
