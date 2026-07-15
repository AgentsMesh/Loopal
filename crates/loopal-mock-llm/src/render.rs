use anyhow::Result;

use crate::protocol::WireProtocol;
use crate::{MockResponse, SseAction};

pub fn plan_protocol_sse(
    protocol: WireProtocol,
    response: &MockResponse,
) -> Result<Vec<SseAction>> {
    if !response.raw_sse.is_empty() {
        return Ok(response
            .raw_sse
            .iter()
            .map(|data| crate::sse::event_raw(data))
            .collect());
    }
    match protocol {
        WireProtocol::Anthropic => crate::sse::plan_sse(response),
        WireProtocol::OpenAiResponses => crate::sse_openai::plan(response),
        WireProtocol::OpenAiCompat => crate::sse_compat::plan(response),
        WireProtocol::Google => crate::sse_google::plan(response),
    }
}
