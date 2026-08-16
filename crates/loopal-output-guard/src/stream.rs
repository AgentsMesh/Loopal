use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroizing;

use crate::Redaction;
use crate::redactor::deduplicate;
use crate::stream_limits::{
    MAX_STREAM_SECRET_BYTES, MAX_STREAM_SECRET_NAME_BYTES, MAX_STREAM_SECRET_PATTERNS,
    MAX_STREAM_SECRET_TOTAL_BYTES, StreamingOutputGuardBuildError, StreamingOutputGuardFinished,
};
use crate::stream_pattern::Pattern;

#[derive(Clone, Copy)]
struct FoundMatch {
    pattern: usize,
    len: usize,
}

pub struct StreamingOutputGuard {
    patterns: Vec<Pattern>,
    pending: Zeroizing<Vec<u8>>,
    matches: Vec<Option<FoundMatch>>,
    offset: usize,
    committed: usize,
    finished: bool,
}

impl StreamingOutputGuard {
    pub fn new(seed: &[(String, SecretString)]) -> Result<Self, StreamingOutputGuardBuildError> {
        if seed.len() > MAX_STREAM_SECRET_PATTERNS {
            return Err(StreamingOutputGuardBuildError);
        }
        let mut patterns: Vec<Pattern> = Vec::new();
        let mut total_bytes = 0usize;
        for (name, value) in seed {
            let plaintext = value.expose_secret();
            if plaintext.is_empty() {
                continue;
            }
            if name.len() > MAX_STREAM_SECRET_NAME_BYTES
                || plaintext.len() > MAX_STREAM_SECRET_BYTES
            {
                return Err(StreamingOutputGuardBuildError);
            }
            if patterns
                .iter()
                .any(|pattern| pattern.value.expose_secret() == plaintext)
            {
                continue;
            }
            total_bytes = total_bytes
                .checked_add(plaintext.len())
                .ok_or(StreamingOutputGuardBuildError)?;
            if total_bytes > MAX_STREAM_SECRET_TOTAL_BYTES {
                return Err(StreamingOutputGuardBuildError);
            }
            patterns.push(Pattern::new(name.clone(), value.clone()));
        }
        let capacity = patterns.iter().map(Pattern::len).max().unwrap_or(0);
        Ok(Self {
            patterns,
            pending: Zeroizing::new(Vec::with_capacity(capacity)),
            matches: Vec::with_capacity(capacity),
            offset: 0,
            committed: 0,
            finished: false,
        })
    }

    pub fn push(
        &mut self,
        chunk: &[u8],
    ) -> Result<Redaction<Vec<u8>>, StreamingOutputGuardFinished> {
        if self.finished {
            return Err(StreamingOutputGuardFinished);
        }
        if self.patterns.is_empty() {
            self.committed += chunk.len();
            return Ok(Redaction::new(chunk.to_vec(), Vec::new()));
        }
        let mut output = Vec::with_capacity(chunk.len());
        let mut names = Vec::new();
        for byte in chunk {
            self.pending.push(*byte);
            self.matches.push(None);
            self.record_matches(*byte);
            self.drain(false, &mut output, &mut names);
            self.compact_if_needed();
        }
        deduplicate(&mut names);
        Ok(Redaction::new(output, names))
    }

    pub fn finish(&mut self) -> Redaction<Vec<u8>> {
        if self.finished {
            return Redaction::new(Vec::new(), Vec::new());
        }
        self.finished = true;
        let mut output = Vec::new();
        let mut names = Vec::new();
        self.drain(true, &mut output, &mut names);
        self.compact();
        deduplicate(&mut names);
        Redaction::new(output, names)
    }

    pub fn committed_input_bytes(&self) -> usize {
        self.committed
    }

    fn record_matches(&mut self, byte: u8) {
        let end = self.pending.len();
        for (index, pattern) in self.patterns.iter_mut().enumerate() {
            if !pattern.advance(byte) {
                continue;
            }
            let len = pattern.len();
            // reason: consume prunes matcher state to the unconsumed suffix, so a
            // completed match cannot start before offset; a later match at the
            // same start is necessarily longer than the one it replaces.
            let start = end - len;
            self.matches[start] = Some(FoundMatch {
                pattern: index,
                len,
            });
        }
    }

    fn drain(&mut self, finishing: bool, output: &mut Vec<u8>, names: &mut Vec<String>) {
        while self.offset < self.pending.len() && (finishing || self.committable_prefix_bytes() > 0)
        {
            if let Some(found) = self.matches[self.offset] {
                let name = &self.patterns[found.pattern].name;
                output.extend_from_slice(b"<secret_ref:");
                output.extend_from_slice(name.as_bytes());
                output.push(b'>');
                names.push(name.clone());
                self.consume(found.len);
            } else {
                output.push(self.pending[self.offset]);
                self.consume(1);
            }
        }
    }

    fn committable_prefix_bytes(&self) -> usize {
        let pending = self.pending.len() - self.offset;
        let protected = self
            .patterns
            .iter()
            .map(Pattern::state)
            .max()
            .unwrap_or(0)
            .min(pending);
        pending.saturating_sub(protected)
    }

    fn consume(&mut self, len: usize) {
        let end = self.offset + len;
        self.pending[self.offset..end].fill(0);
        self.matches[self.offset..end].fill(None);
        self.offset = end;
        self.committed += len;
        let remaining = self.pending.len() - self.offset;
        for pattern in &mut self.patterns {
            pattern.prune(remaining);
        }
    }

    fn compact_if_needed(&mut self) {
        if self.pending.len() == self.pending.capacity() || self.offset >= 8 * 1024 {
            self.compact();
        }
    }

    fn compact(&mut self) {
        if self.offset == 0 {
            return;
        }
        let remaining = self.pending.len() - self.offset;
        self.pending.copy_within(self.offset.., 0);
        self.pending[remaining..].fill(0);
        self.pending.truncate(remaining);
        self.matches.copy_within(self.offset.., 0);
        self.matches.truncate(remaining);
        self.offset = 0;
    }
}

impl std::fmt::Debug for StreamingOutputGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StreamingOutputGuard")
            .field("pattern_count", &self.patterns.len())
            .field("pending_bytes", &(self.pending.len() - self.offset))
            .field("finished", &self.finished)
            .finish()
    }
}
