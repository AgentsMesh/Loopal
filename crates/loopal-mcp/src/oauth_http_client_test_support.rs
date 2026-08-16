use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use reqwest::header::{HeaderName, HeaderValue};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};

type ObservedCalls = Arc<Mutex<Vec<(&'static str, Option<String>)>>>;

#[derive(Clone)]
pub(super) struct TestClient {
    posts: Arc<Mutex<Vec<Result<StreamableHttpPostResponse, String>>>>,
    pub(super) observed: ObservedCalls,
}

impl TestClient {
    pub(super) fn new(posts: Vec<Result<StreamableHttpPostResponse, String>>) -> Self {
        Self {
            posts: Arc::new(Mutex::new(posts)),
            observed: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record(&self, method: &'static str, token: Option<String>) {
        self.observed.lock().unwrap().push((method, token));
    }
}

impl StreamableHttpClient for TestClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        _: Arc<str>,
        _: ClientJsonRpcMessage,
        _: Option<Arc<str>>,
        token: Option<String>,
        _: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.record("post", token);
        self.posts
            .lock()
            .unwrap()
            .remove(0)
            .map_err(|message| StreamableHttpError::UnexpectedServerResponse(message.into()))
    }

    async fn delete_session(
        &self,
        _: Arc<str>,
        _: Arc<str>,
        token: Option<String>,
        _: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        self.record("delete", token);
        Ok(())
    }

    async fn get_stream(
        &self,
        _: Arc<str>,
        _: Arc<str>,
        _: Option<String>,
        token: Option<String>,
        _: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        self.record("get", token);
        Ok(futures::stream::empty().boxed())
    }
}

pub(super) fn ping() -> ClientJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "ping"
    }))
    .unwrap()
}

pub(super) fn error_message(secret: &str) -> ServerJsonRpcMessage {
    serde_json::from_value(serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "error": {
            "code": -32000,
            "message": format!("server echoed {secret}"),
            "data": {secret: format!("Bearer {secret}")}
        }
    }))
    .unwrap()
}
