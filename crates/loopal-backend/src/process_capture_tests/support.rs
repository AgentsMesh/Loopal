use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

use async_trait::async_trait;
use loopal_output_guard::{OutputGuard, StreamingOutputGuard};
use loopal_tool_api::{ProcessOutputSanitizer, ProcessOutputStream};
use parking_lot::Mutex;
use secrecy::SecretString;
use tokio::io::{AsyncRead, ReadBuf};

use crate::process_capture::CaptureReader;
use crate::process_capture_io::CaptureSink;

pub(super) struct TestSink {
    bytes: Mutex<Vec<u8>>,
    fail_write: bool,
    fail_flush_at: Option<usize>,
    flushes: AtomicUsize,
}

impl TestSink {
    pub fn new(fail_write: bool, fail_flush_at: Option<usize>) -> Arc<Self> {
        Arc::new(Self {
            bytes: Mutex::new(Vec::new()),
            fail_write,
            fail_flush_at,
            flushes: AtomicUsize::new(0),
        })
    }

    pub fn text(&self) -> String {
        String::from_utf8(self.bytes.lock().clone()).unwrap()
    }
}

#[async_trait]
impl CaptureSink for TestSink {
    async fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        if self.fail_write {
            return Err(io::Error::other("injected write failure"));
        }
        self.bytes.lock().extend_from_slice(bytes);
        Ok(())
    }

    async fn flush(&self) -> io::Result<()> {
        let call = self.flushes.fetch_add(1, Ordering::Relaxed) + 1;
        if self.fail_flush_at == Some(call) {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}

pub(super) struct TestReader {
    steps: VecDeque<io::Result<Vec<u8>>>,
    pending_once: bool,
}

impl TestReader {
    pub fn chunks(chunks: impl IntoIterator<Item = Vec<u8>>) -> CaptureReader {
        Self::reader(chunks, false)
    }

    pub fn delayed(chunks: impl IntoIterator<Item = Vec<u8>>) -> CaptureReader {
        Self::reader(chunks, true)
    }

    fn reader(chunks: impl IntoIterator<Item = Vec<u8>>, pending_once: bool) -> CaptureReader {
        Box::new(Self {
            steps: chunks.into_iter().map(Ok).collect(),
            pending_once,
        })
    }

    pub fn failing() -> CaptureReader {
        Box::new(Self {
            steps: VecDeque::from([Err(io::Error::other("injected read failure"))]),
            pending_once: false,
        })
    }
}

impl AsyncRead for TestReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.pending_once {
            self.pending_once = false;
            _context.waker().wake_by_ref();
            return Poll::Pending;
        }
        match self.steps.pop_front() {
            Some(Ok(mut bytes)) => {
                let take = bytes.len().min(buffer.remaining());
                let remainder = bytes.split_off(take);
                buffer.put_slice(&bytes);
                if !remainder.is_empty() {
                    self.steps.push_front(Ok(remainder));
                }
                Poll::Ready(Ok(()))
            }
            Some(Err(error)) => Poll::Ready(Err(error)),
            None => Poll::Ready(Ok(())),
        }
    }
}

pub(super) fn sanitizer(plaintext: &str) -> Arc<dyn ProcessOutputSanitizer> {
    Arc::new(GuardFactory(vec![(
        "token".into(),
        SecretString::from(plaintext.to_string()),
    )]))
}

struct GuardFactory(Vec<(String, SecretString)>);

impl ProcessOutputSanitizer for GuardFactory {
    fn stream(&self) -> Box<dyn ProcessOutputStream> {
        Box::new(GuardStream(StreamingOutputGuard::new(&self.0).unwrap()))
    }

    fn guard_text(&self, text: &str) -> String {
        OutputGuard::new(&self.0)
            .unwrap()
            .redact_text(text)
            .into_inner()
    }
}

struct GuardStream(StreamingOutputGuard);

impl ProcessOutputStream for GuardStream {
    fn sanitize(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.0.push(chunk).unwrap().into_inner()
    }

    fn finish(&mut self) -> Vec<u8> {
        self.0.finish().into_inner()
    }

    fn committed_input_bytes(&self) -> usize {
        self.0.committed_input_bytes()
    }
}
