/// MCP client wrapping rmcp's `RunningService`.
///
/// Provides typed methods for tools/resources/prompts with timeout enforcement.
use std::sync::Arc;
use std::time::Duration;

use loopal_error::McpError;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientRequest, ListPromptsResult, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult,
    Request, RequestOptionalParam, ServerResult,
};
use rmcp::service::{PeerRequestOptions, RoleClient, RunningService, ServiceError, ServiceExt};
use tracing::info;

use crate::handler::{LoopalClientHandler, SamplingCallback};

/// A connected MCP client backed by rmcp.
pub struct McpClient {
    service: RunningService<RoleClient, LoopalClientHandler>,
    timeout: Duration,
}

impl McpClient {
    /// Connect to an MCP server over any transport.
    ///
    /// Performs the MCP handshake (`initialize` / `initialized`) and returns a
    /// ready-to-use client. Pass a `SamplingCallback` to enable server-initiated
    /// LLM calls; pass `None` to disable sampling.
    pub async fn connect<T, E, A>(
        transport: T,
        timeout: Duration,
        sampling: Option<Arc<dyn SamplingCallback>>,
    ) -> Result<Self, McpError>
    where
        T: rmcp::transport::IntoTransport<RoleClient, E, A>,
        E: std::error::Error + From<std::io::Error> + Send + Sync + 'static,
    {
        let handler = LoopalClientHandler::new(sampling);
        let service = handler
            .serve(transport)
            .await
            .map_err(|e| McpError::ConnectionFailed(e.to_string()))?;

        if let Some(info) = service.peer_info() {
            info!(
                server = %info.server_info.name,
                version = %info.server_info.version,
                protocol = ?info.protocol_version,
                "MCP server connected"
            );
        }

        Ok(Self { service, timeout })
    }

    /// List tools the server exposes.
    pub async fn list_tools(&self) -> Result<ListToolsResult, McpError> {
        let req = ClientRequest::ListToolsRequest(RequestOptionalParam::with_param(
            PaginatedRequestParams::default(),
        ));
        match self.send(req).await? {
            ServerResult::ListToolsResult(r) => Ok(r),
            _ => Err(McpError::Protocol(
                "unexpected response to tools/list".into(),
            )),
        }
    }

    /// Call a tool by name with JSON arguments.
    pub async fn call_tool(
        &self,
        name: &str,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        let params = CallToolRequestParams::new(name.to_string()).with_arguments(args);
        let req = ClientRequest::CallToolRequest(Request::new(params));
        match self.send(req).await? {
            ServerResult::CallToolResult(r) => Ok(r),
            _ => Err(McpError::Protocol(
                "unexpected response to tools/call".into(),
            )),
        }
    }

    /// List resources the server exposes.
    pub async fn list_resources(&self) -> Result<ListResourcesResult, McpError> {
        let req = ClientRequest::ListResourcesRequest(RequestOptionalParam::with_param(
            PaginatedRequestParams::default(),
        ));
        match self.send(req).await? {
            ServerResult::ListResourcesResult(r) => Ok(r),
            _ => Err(McpError::Protocol(
                "unexpected response to resources/list".into(),
            )),
        }
    }

    /// Read a specific resource by URI.
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, McpError> {
        let req = ClientRequest::ReadResourceRequest(Request::new(ReadResourceRequestParams::new(
            uri.to_string(),
        )));
        match self.send(req).await? {
            ServerResult::ReadResourceResult(r) => Ok(r),
            _ => Err(McpError::Protocol(
                "unexpected response to resources/read".into(),
            )),
        }
    }

    /// List prompts the server exposes.
    pub async fn list_prompts(&self) -> Result<ListPromptsResult, McpError> {
        let req = ClientRequest::ListPromptsRequest(RequestOptionalParam::with_param(
            PaginatedRequestParams::default(),
        ));
        match self.send(req).await? {
            ServerResult::ListPromptsResult(r) => Ok(r),
            _ => Err(McpError::Protocol(
                "unexpected response to prompts/list".into(),
            )),
        }
    }

    /// Access the server's capability info from the initialize handshake.
    pub fn peer_info(&self) -> Option<&rmcp::model::InitializeResult> {
        self.service.peer_info()
    }

    /// Check if the underlying transport is closed.
    pub fn is_closed(&self) -> bool {
        self.service.is_closed()
    }

    /// Send a request with timeout. Uses rmcp's built-in timeout on RequestHandle.
    async fn send(&self, request: ClientRequest) -> Result<ServerResult, McpError> {
        let mut options = PeerRequestOptions::default();
        options.timeout = Some(self.timeout);

        let handle = self
            .service
            .send_cancellable_request(request, options)
            .await
            .map_err(|e| McpError::TransportClosed(e.to_string()))?;

        handle.await_response().await.map_err(|e| match e {
            ServiceError::Timeout { timeout } => {
                McpError::Timeout(format!("{}ms", timeout.as_millis()))
            }
            ServiceError::TransportClosed => McpError::TransportClosed("transport closed".into()),
            other => McpError::Protocol(other.to_string()),
        })
    }
}
