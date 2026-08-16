use loopal_secret_client::SecretString;
use rmcp::RoleClient;
use rmcp::model::{
    ClientJsonRpcMessage, ClientRequest, ErrorData, Implementation, ProtocolVersion, RequestId,
    ServerJsonRpcMessage, ServerResult,
};
use rmcp::transport::Transport;

use crate::result_sanitizer::CallResultSanitizer;

#[derive(Clone)]
pub(crate) enum HandshakePolicy {
    Redact(std::sync::Arc<CallResultSanitizer>),
    Strip,
}

impl HandshakePolicy {
    pub(crate) fn from_seed(seed: &[(String, SecretString)]) -> Self {
        Self::Redact(std::sync::Arc::new(CallResultSanitizer::new(seed)))
    }

    pub(crate) fn accepts_opaque_text(&self, value: &str) -> bool {
        match self {
            Self::Redact(sanitizer) => sanitizer.sanitize_text(value) == value,
            Self::Strip => true,
        }
    }
}

pub(crate) struct HandshakeSanitizingTransport<T> {
    inner: T,
    policy: Option<HandshakePolicy>,
    request_id: Option<RequestId>,
}

impl<T> HandshakeSanitizingTransport<T> {
    pub(crate) fn new(inner: T, policy: HandshakePolicy) -> Self {
        Self {
            inner,
            policy: Some(policy),
            request_id: None,
        }
    }
}

impl<T> Transport<RoleClient> for HandshakeSanitizingTransport<T>
where
    T: Transport<RoleClient>,
{
    type Error = T::Error;

    fn send(
        &mut self,
        item: ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        if let ClientJsonRpcMessage::Request(request) = &item
            && matches!(request.request, ClientRequest::InitializeRequest(_))
        {
            self.request_id = Some(request.id.clone());
        }
        self.inner.send(item)
    }

    async fn receive(&mut self) -> Option<ServerJsonRpcMessage> {
        loop {
            let mut message = self.inner.receive().await?;
            let Some(policy) = self.policy.as_ref() else {
                return Some(message);
            };
            if sanitize_handshake_message(&mut message, policy, self.request_id.as_ref()) {
                self.policy = None;
                return Some(message);
            }
        }
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.inner.close()
    }
}

pub(crate) fn sanitize_handshake_message(
    message: &mut ServerJsonRpcMessage,
    policy: &HandshakePolicy,
    request_id: Option<&RequestId>,
) -> bool {
    match message {
        ServerJsonRpcMessage::Response(response) => {
            sanitize_id(&mut response.id, request_id);
            let valid = match &mut response.result {
                ServerResult::InitializeResult(info) => sanitize_info(info, policy),
                _ => false,
            };
            if !valid {
                response.result = ServerResult::empty(());
            }
            true
        }
        ServerJsonRpcMessage::Error(error) => {
            if let Some(id) = &mut error.id {
                sanitize_id(id, request_id);
            }
            sanitize_error(&mut error.error);
            true
        }
        _ => false,
    }
}

fn sanitize_info(info: &mut rmcp::model::InitializeResult, policy: &HandshakePolicy) -> bool {
    let Some(version) = ProtocolVersion::KNOWN_VERSIONS
        .iter()
        .find(|version| version.as_str() == info.protocol_version.as_str())
    else {
        return false;
    };
    info.protocol_version = version.clone();
    info.capabilities.experimental = None;
    info.capabilities.extensions = None;
    info.capabilities.logging = info.capabilities.logging.take().map(|_| Default::default());
    info.capabilities.completions = info
        .capabilities
        .completions
        .take()
        .map(|_| Default::default());
    info.capabilities.tasks = None;
    match policy {
        HandshakePolicy::Redact(sanitizer) => {
            info.server_info.name = sanitizer.sanitize_text(&info.server_info.name);
            info.server_info.version = sanitizer.sanitize_text(&info.server_info.version);
            info.instructions = info
                .instructions
                .as_deref()
                .map(|value| sanitizer.sanitize_text(value));
        }
        HandshakePolicy::Strip => {
            info.server_info = Implementation::new("MCP server", "");
            info.instructions = None;
        }
    }
    info.server_info.title = None;
    info.server_info.description = None;
    info.server_info.icons = None;
    info.server_info.website_url = None;
    true
}

fn sanitize_id(id: &mut RequestId, expected: Option<&RequestId>) {
    if expected != Some(id) {
        *id = RequestId::Number(-1);
    }
}

fn sanitize_error(error: &mut ErrorData) {
    let message = error.message.to_ascii_lowercase();
    error.message = if message.contains("auth") || message.contains("401") {
        "MCP authentication required".into()
    } else {
        "MCP handshake failed".into()
    };
    error.data = None;
}

#[cfg(test)]
#[path = "handshake_transport_tests.rs"]
mod tests;
