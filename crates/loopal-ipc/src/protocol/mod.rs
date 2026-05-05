//! IPC protocol method definitions.
//!
//! Maps the agent communication to JSON-RPC methods.
//! Each method corresponds to a message type that crosses the process boundary.

pub mod methods;

/// A protocol method with its name string.
pub struct Method {
    pub name: &'static str,
}
