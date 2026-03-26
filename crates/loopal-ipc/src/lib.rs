//! IPC transport and protocol layer for Loopal multi-process architecture.
//!
//! Provides a platform-abstracted transport layer (Mojo-like) and JSON-RPC 2.0
//! protocol for communication between TUI, Agent, and Sub-Agent processes.

pub mod connection;
pub mod jsonrpc;
pub mod protocol;
pub mod stdio;
pub mod transport;

pub use connection::Connection;
pub use jsonrpc::{IncomingMessage, JsonRpcError, read_message};
pub use protocol::{Method, methods};
pub use stdio::StdioTransport;
pub use transport::Transport;
