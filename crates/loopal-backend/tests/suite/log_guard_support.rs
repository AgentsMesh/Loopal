use std::sync::Arc;

use loopal_output_guard::{OutputGuard, StreamingOutputGuard};
use loopal_tool_api::{ProcessOutputSanitizer, ProcessOutputStream};
use secrecy::SecretString;

pub struct GuardFactory(Vec<(String, SecretString)>);

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

pub fn process_sanitizer(plaintext: &str) -> Arc<dyn ProcessOutputSanitizer> {
    Arc::new(GuardFactory(vec![(
        "token".into(),
        SecretString::from(plaintext.to_string()),
    )]))
}
