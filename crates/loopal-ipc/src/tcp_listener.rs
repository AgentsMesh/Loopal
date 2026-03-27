//! TCP listener for accepting IPC client connections.
//!
//! Binds to `127.0.0.1` with a system-assigned port and generates a random
//! token for authentication. Clients must present the token during the
//! `initialize` handshake.

use std::net::SocketAddr;

use loopal_error::LoopalError;
use tokio::net::TcpListener;

use crate::tcp::TcpTransport;

/// A TCP listener that accepts IPC connections on localhost.
///
/// The listener generates a random token at creation time. Callers are
/// responsible for communicating the port and token to potential clients
/// (e.g. by writing them to a well-known file).
pub struct IpcListener {
    listener: TcpListener,
    token: String,
}

impl IpcListener {
    /// Bind to the given address and generate a random auth token.
    ///
    /// Use `127.0.0.1:0` to let the OS assign an available port.
    pub async fn bind(addr: SocketAddr) -> Result<Self, LoopalError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| LoopalError::Ipc(format!("TCP bind failed: {e}")))?;
        let token = generate_token();
        Ok(Self { listener, token })
    }

    /// The port this listener is bound to.
    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .expect("listener should have a local address")
            .port()
    }

    /// The authentication token clients must present during `initialize`.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Accept the next incoming TCP connection.
    pub async fn accept(&self) -> Result<(TcpTransport, SocketAddr), LoopalError> {
        let (stream, addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| LoopalError::Ipc(format!("TCP accept failed: {e}")))?;
        Ok((TcpTransport::new(stream), addr))
    }
}

fn generate_token() -> String {
    uuid::Uuid::new_v4().to_string()
}
