//! IPC transport and protocol layer for Loopal multi-process architecture.
//!
//! Provides a platform-abstracted transport layer (Mojo-like) and JSON-RPC 2.0
//! protocol for communication between consumer, agent, and sub-agent processes.

pub mod budget;
pub mod connection;
mod connection_reader;
pub mod cross_hub;
pub mod desktop_handshake;
pub mod dispatcher;
pub mod duplex;
mod frame;
pub mod handshake_protocol;
pub mod jsonrpc;
pub mod protocol;
pub mod rpc_error;
pub mod stdio;
pub mod tcp;
pub mod tcp_listener;
pub mod transport;

pub use budget::{HUB_RPC_BUDGET, IpcBudget};
pub use connection::{Connection, Inactive, Listening};
pub use desktop_handshake::{
    DESKTOP_CAPABILITY_HUB_UI, DESKTOP_CAPABILITY_WORKSPACE, DESKTOP_EVENT_PREFIX,
    DESKTOP_HANDSHAKE_PREFIX, DESKTOP_PROTOCOL_VERSION, DESKTOP_TRANSPORT, DesktopHandshake,
    DesktopHandshakeEvent,
};
pub use dispatcher::{Dispatcher, DispatcherBuilder, HandlerCtx, RequestHandler};
pub use duplex::duplex_pair;
pub use frame::MAX_IPC_FRAME_BYTES;
pub use handshake_protocol::HandshakeLine;
pub use jsonrpc::{IncomingMessage, JsonRpcError, read_message};
pub use protocol::{Method, methods};
pub use rpc_error::RpcError;
pub use stdio::StdioTransport;
pub use tcp::TcpTransport;
pub use tcp_listener::IpcListener;
pub use transport::Transport;
