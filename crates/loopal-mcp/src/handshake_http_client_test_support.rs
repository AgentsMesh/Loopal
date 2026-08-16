use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use reqwest::header::{HeaderName, HeaderValue};
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::common::client_side_sse::BoxedSseResponse;
use rmcp::transport::streamable_http_client::{
    StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
};

#[derive(Clone)]
pub(crate) struct FakeHttpClient {
    posts: Arc<Mutex<Vec<Result<StreamableHttpPostResponse, String>>>>,
    pub(crate) deletes: Arc<Mutex<usize>>,
    pub(crate) gets: Arc<Mutex<usize>>,
}

impl FakeHttpClient {
    pub(crate) fn new(posts: Vec<Result<StreamableHttpPostResponse, String>>) -> Self {
        Self {
            posts: Arc::new(Mutex::new(posts)),
            deletes: Arc::new(Mutex::new(0)),
            gets: Arc::new(Mutex::new(0)),
        }
    }
}

impl StreamableHttpClient for FakeHttpClient {
    type Error = reqwest::Error;

    async fn post_message(
        &self,
        _: Arc<str>,
        _: ClientJsonRpcMessage,
        _: Option<Arc<str>>,
        _: Option<String>,
        _: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        let next = self.posts.lock().unwrap().remove(0);
        next.map_err(|message| StreamableHttpError::UnexpectedServerResponse(message.into()))
    }

    async fn delete_session(
        &self,
        _: Arc<str>,
        _: Arc<str>,
        _: Option<String>,
        _: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        *self.deletes.lock().unwrap() += 1;
        Ok(())
    }

    async fn get_stream(
        &self,
        _: Arc<str>,
        _: Arc<str>,
        _: Option<String>,
        _: Option<String>,
        _: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxedSseResponse, StreamableHttpError<Self::Error>> {
        *self.gets.lock().unwrap() += 1;
        Ok(futures::stream::empty().boxed())
    }
}
