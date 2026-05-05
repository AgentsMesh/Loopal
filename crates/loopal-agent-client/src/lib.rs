//! Agent client for multi-process architecture.
//!
//! Used by the consumer process (or a parent agent) to spawn and communicate with
//! an Agent process. This is the "Browser Process" side in the Chromium analogy.

pub mod bridge;
mod client;
mod process;
pub(crate) mod stderr_drain;

#[doc(hidden)]
pub mod test_support {
    pub use crate::stderr_drain::drain_to_tracing;
}

pub use bridge::{BridgeHandles, start_bridge};
pub use client::{AgentClient, AgentClientEvent, StartAgentParams};
pub use process::AgentProcess;
