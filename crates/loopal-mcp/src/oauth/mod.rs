//! OAuth support for MCP HTTP connections.
//!
//! Handles the browser-based authorization flow:
//! 1. Detect `AuthRequired` error on initial HTTP connection
//! 2. Discover OAuth metadata from server
//! 3. Open browser for user authorization
//! 4. Receive callback with authorization code
//! 5. Exchange code for access token
//! 6. Reconnect with `AuthClient` wrapping the HTTP client

pub mod callback;
pub mod flow;
pub mod store;
