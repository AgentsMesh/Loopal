pub mod client;
mod client_connect;
mod client_oauth;
pub mod connection;
mod connection_discovery;
mod connection_generation;
mod contained_stdio_transport;
pub mod handler;
mod handshake_http_client;
mod handshake_transport;
mod http_redirect_policy;
#[cfg(test)]
#[path = "http_test_support_tests.rs"]
mod http_test_support;
pub mod local_provider;
pub mod manager;
mod manager_prepare;
mod manager_query;
#[cfg(test)]
mod manager_query_tests;
mod manager_reconnect;
#[cfg(test)]
mod manager_secret_tests;
pub mod oauth;
mod oauth_credential_seed;
mod oauth_http_client;
mod oauth_http_sanitize;
mod oauth_transport;
pub mod provider;
#[cfg(test)]
mod provider_call_tests;
pub mod proxy_client;
mod proxy_reconnect;
pub mod reconnect;
mod resolved_config;
pub mod result_sanitizer;
#[cfg(test)]
mod result_sanitizer_tests;
mod safe_diagnostics;
mod scoped_http_client;
mod secret_expand;
#[cfg(test)]
mod secret_expand_rejection_tests;
#[cfg(test)]
#[path = "secret_expand_support_tests.rs"]
mod secret_expand_test_support;
#[cfg(test)]
mod secret_expand_tests;
mod secret_provenance;
mod settle_signal;
mod stdio_command;
pub mod tool_adapter;
pub mod tool_result_text;
pub mod transport;
pub mod types;

pub use client::McpClient;
pub use connection::McpConnection;
pub use handler::SamplingCallback;
pub use local_provider::LocalMcpProvider;
pub use loopal_ipc::{HUB_RPC_BUDGET, IpcBudget};
pub use manager::McpManager;
pub use manager_query::McpConnectionSnapshot;
pub use provider::McpProvider;
pub use proxy_client::{HubMcpClient, McpProxyClient};
pub use result_sanitizer::{BINARY_DENIED_MARKER, CallResultSanitizer};
pub use tool_adapter::McpToolAdapter;
pub use tool_result_text::call_result_to_response;
pub use types::ConnectionStatus;
