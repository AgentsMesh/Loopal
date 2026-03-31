//! IPC transport and protocol layer for Loopal multi-process architecture.
//!
//! Provides a platform-abstracted transport layer (Mojo-like) and JSON-RPC 2.0
//! protocol for communication between consumer, agent, and sub-agent processes.

pub mod connection;
pub mod duplex;
pub mod jsonrpc;
pub mod protocol;
pub mod stdio;
pub mod tcp;
pub mod tcp_listener;
pub mod transport;

pub use connection::Connection;
pub use duplex::duplex_pair;
pub use jsonrpc::{IncomingMessage, JsonRpcError, read_message};
pub use protocol::{Method, methods};
pub use stdio::StdioTransport;
pub use tcp::TcpTransport;
pub use tcp_listener::IpcListener;
pub use transport::Transport;
