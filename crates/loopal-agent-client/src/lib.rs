//! Agent client for multi-process architecture.
//!
//! Used by the consumer process (or a parent agent) to spawn and communicate with
//! an Agent process. This is the "Browser Process" side in the Chromium analogy.

pub mod bridge;
mod bridge_handlers;
mod client;
mod process;

pub use bridge::{BridgeHandles, start_bridge};
pub use client::{AgentClient, AgentClientEvent};
pub use process::AgentProcess;
