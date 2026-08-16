use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use secrecy::SecretString;

use loopal_output_guard::StreamingOutputGuard;
use loopal_tool_api::{ProcessOutputSanitizer, ProcessOutputStream};

pub(crate) struct SecretProcessOutputSanitizer {
    tool_name: String,
    session_id: String,
    seed: Vec<(String, SecretString)>,
    audited: Arc<Mutex<HashSet<String>>>,
}

impl SecretProcessOutputSanitizer {
    pub(crate) fn new(
        tool_name: &str,
        session_id: &str,
        seed: &[(String, SecretString)],
    ) -> loopal_error::Result<Self> {
        if StreamingOutputGuard::new(seed).is_err() {
            return Err(loopal_error::LoopalError::Other(
                "secret set exceeds streaming redaction limits".into(),
            ));
        }
        Ok(Self {
            tool_name: tool_name.to_string(),
            session_id: session_id.to_string(),
            seed: seed.to_vec(),
            audited: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}

impl ProcessOutputSanitizer for SecretProcessOutputSanitizer {
    fn stream(&self) -> Box<dyn ProcessOutputStream> {
        Box::new(SecretProcessOutputStream {
            tool_name: self.tool_name.clone(),
            session_id: self.session_id.clone(),
            guard: StreamingOutputGuard::new(&self.seed).expect("seed validated at construction"),
            audited: self.audited.clone(),
        })
    }

    fn guard_text(&self, text: &str) -> String {
        let redactor = loopal_secret_runtime::Redactor::from_pairs(&self.seed);
        let (redacted, names) = redactor.scan_and_redact(text);
        audit_names(&self.audited, &self.tool_name, &self.session_id, &names);
        redacted
    }
}

struct SecretProcessOutputStream {
    tool_name: String,
    session_id: String,
    guard: StreamingOutputGuard,
    audited: Arc<Mutex<HashSet<String>>>,
}

fn audit_names(
    audited: &Mutex<HashSet<String>>,
    tool_name: &str,
    session_id: &str,
    names: &[String],
) {
    let fresh = {
        let mut audited = audited.lock().unwrap();
        names
            .iter()
            .filter(|name| audited.insert((*name).clone()))
            .cloned()
            .collect::<Vec<_>>()
    };
    loopal_secret_runtime::record_redaction_hits(tool_name, &fresh, session_id);
}

impl SecretProcessOutputStream {
    fn audit(&self, names: &[String]) {
        audit_names(&self.audited, &self.tool_name, &self.session_id, names);
    }
}

impl ProcessOutputStream for SecretProcessOutputStream {
    fn sanitize(&mut self, chunk: &[u8]) -> Vec<u8> {
        let redacted = self
            .guard
            .push(chunk)
            .expect("stream is not reused after EOF");
        self.audit(redacted.secret_names());
        redacted.into_inner()
    }

    fn finish(&mut self) -> Vec<u8> {
        let redacted = self.guard.finish();
        self.audit(redacted.secret_names());
        redacted.into_inner()
    }

    fn committed_input_bytes(&self) -> usize {
        self.guard.committed_input_bytes()
    }
}

#[cfg(test)]
#[path = "process_output_sanitizer/tests.rs"]
mod tests;
