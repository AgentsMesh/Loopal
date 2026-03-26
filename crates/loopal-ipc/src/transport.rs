//! Abstract transport trait for cross-platform IPC.
//!
//! Inspired by Chromium Mojo's MessagePipe concept but drastically simplified.
//! Implementations provide bidirectional byte-stream communication; the protocol
//! layer (`Connection`) handles JSON-RPC framing on top.
//!
//! Current implementations:
//! - `StdioTransport` — stdin/stdout pipes (cross-platform, used by default)
//!
//! Future implementations:
//! - `UnixSocketTransport` — Unix domain sockets (reconnection support)
//! - `TcpLoopbackTransport` — localhost TCP (Windows compatibility)

use async_trait::async_trait;
use loopal_error::LoopalError;

/// A bidirectional byte-stream transport for IPC communication.
///
/// Implementations must be `Send + Sync` to allow sharing across async tasks.
/// Each call to `send` writes one complete message frame; each call to `recv`
/// reads one complete message frame. Framing semantics (e.g. newline-delimited)
/// are defined by the implementation.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Send a complete message frame.
    async fn send(&self, data: &[u8]) -> Result<(), LoopalError>;

    /// Receive the next complete message frame. Returns `None` on EOF / disconnect.
    async fn recv(&self) -> Result<Option<Vec<u8>>, LoopalError>;

    /// Check whether the transport is still connected.
    fn is_connected(&self) -> bool;
}
