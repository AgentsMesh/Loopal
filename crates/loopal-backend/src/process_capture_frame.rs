use std::collections::VecDeque;

use zeroize::Zeroizing;

use crate::process_capture_preview::CaptureSource;

pub(crate) enum PreviewAction {
    Data(CaptureSource, Zeroizing<Vec<u8>>),
    Finish(CaptureSource),
}

pub(crate) struct Framer {
    stderr_line_start: bool,
    input_bytes: usize,
    pending: VecDeque<(usize, PreviewAction)>,
}

impl Framer {
    pub fn new() -> Self {
        Self {
            stderr_line_start: true,
            input_bytes: 0,
            pending: VecDeque::new(),
        }
    }

    pub fn frame(
        &mut self,
        source: CaptureSource,
        source_bytes: Zeroizing<Vec<u8>>,
    ) -> Zeroizing<Vec<u8>> {
        if source_bytes.is_empty() {
            return Zeroizing::new(Vec::new());
        }
        let framed = match source {
            CaptureSource::Stdout => Zeroizing::new(source_bytes.as_slice().to_vec()),
            CaptureSource::Stderr => self.frame_stderr(&source_bytes),
        };
        self.input_bytes += framed.len();
        self.pending
            .push_back((self.input_bytes, PreviewAction::Data(source, source_bytes)));
        framed
    }

    pub fn finish_source(&mut self, source: CaptureSource) {
        self.pending
            .push_back((self.input_bytes, PreviewAction::Finish(source)));
    }

    pub fn take_committed(&mut self, committed: usize) -> Vec<PreviewAction> {
        let mut actions = Vec::new();
        while self
            .pending
            .front()
            .is_some_and(|(end, _)| *end <= committed)
        {
            if let Some((_, action)) = self.pending.pop_front() {
                actions.push(action);
            }
        }
        actions
    }

    fn frame_stderr(&mut self, bytes: &[u8]) -> Zeroizing<Vec<u8>> {
        let mut framed = Zeroizing::new(Vec::with_capacity(bytes.len() + 16));
        for segment in bytes.split_inclusive(|byte| *byte == b'\n') {
            if self.stderr_line_start {
                framed.extend_from_slice(b"[err] ");
            }
            framed.extend_from_slice(segment);
            self.stderr_line_start = segment.ends_with(b"\n");
        }
        framed
    }
}

pub(crate) struct Utf8Decoder {
    pending: Zeroizing<Vec<u8>>,
}

impl Utf8Decoder {
    pub fn new() -> Self {
        Self {
            pending: Zeroizing::new(Vec::with_capacity(4)),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Zeroizing<Vec<u8>> {
        let mut input = Zeroizing::new(Vec::with_capacity(self.pending.len() + bytes.len()));
        input.extend_from_slice(&self.pending);
        input.extend_from_slice(bytes);
        self.pending.clear();
        self.decode(input, false)
    }

    pub fn finish(&mut self) -> Zeroizing<Vec<u8>> {
        let input = Zeroizing::new(std::mem::take(&mut *self.pending));
        self.decode(input, true)
    }

    fn decode(&mut self, input: Zeroizing<Vec<u8>>, finishing: bool) -> Zeroizing<Vec<u8>> {
        let mut output = Zeroizing::new(Vec::with_capacity(input.len()));
        let mut offset = 0;
        while offset < input.len() {
            match std::str::from_utf8(&input[offset..]) {
                Ok(valid) => {
                    output.extend_from_slice(valid.as_bytes());
                    break;
                }
                Err(error) => {
                    let valid_end = offset + error.valid_up_to();
                    output.extend_from_slice(&input[offset..valid_end]);
                    let Some(error_len) = error.error_len() else {
                        if finishing {
                            output.extend_from_slice("�".as_bytes());
                        } else {
                            self.pending.extend_from_slice(&input[valid_end..]);
                        }
                        break;
                    };
                    output.extend_from_slice("�".as_bytes());
                    offset = valid_end + error_len;
                }
            }
        }
        output
    }
}
