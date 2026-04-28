//! Collaboration tools — Hub-based multi-agent coordination.
//!
//! - **Agent**: spawn sub-agents (foreground or background, local or cross-hub)
//! - **SendMessage**: point-to-point message routing via Hub
//! - **ListHubs**: discover other hubs in MetaHub cluster

pub mod agent;
mod agent_fork;
mod agent_spawn;
pub mod list_hubs;
pub mod send_message;
pub(crate) mod shared_extract;
mod spawn_decision;
