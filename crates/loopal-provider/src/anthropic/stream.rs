pub(crate) use super::accumulator::{
    ServerToolAccumulator, ThinkingAccumulator, ToolUseAccumulator,
};

use futures::stream::Stream;
use loopal_error::LoopalError;
use loopal_provider_api::StreamChunk;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::stream_parser::parse_anthropic_event;

pub(crate) struct AnthropicStream {
    pub(crate) inner: Pin<Box<dyn Stream<Item = Result<String, LoopalError>> + Send>>,
    pub(crate) tool_state: ToolUseAccumulator,
    pub(crate) thinking_state: ThinkingAccumulator,
    pub(crate) server_tool_state: ServerToolAccumulator,
    pub(crate) buffer: VecDeque<Result<StreamChunk, LoopalError>>,
}

impl Stream for AnthropicStream {
    type Item = Result<StreamChunk, LoopalError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(item) = this.buffer.pop_front() {
            return Poll::Ready(Some(item));
        }

        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(data))) => {
                let chunks = parse_anthropic_event(
                    &data,
                    &mut this.tool_state,
                    &mut this.thinking_state,
                    &mut this.server_tool_state,
                );
                let mut iter = chunks.into_iter();
                if let Some(first) = iter.next() {
                    this.buffer.extend(iter);
                    Poll::Ready(Some(first))
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// SAFETY: All fields are Send
unsafe impl Send for AnthropicStream {}
impl Unpin for AnthropicStream {}
