use std::sync::Arc;

use loopal_error::ToolIoError;
use loopal_tool_api::{OutputTail, ProcessOutputSanitizer, ProcessOutputStream};
use tokio::io::AsyncRead;
use tokio::process::{ChildStderr, ChildStdout};
use zeroize::Zeroizing;

use crate::log_writer::LogWriter;
use crate::process_capture_frame::{Framer, PreviewAction, Utf8Decoder};
use crate::process_capture_io::{CaptureSink, LogCaptureSink, capture_error, persist, read_pipe};
use crate::process_capture_preview::{CaptureSource, ProcessPreviews};
use crate::process_capture_state::ProcessCaptureState;
use crate::process_capture_task::ProcessCaptureTask;

const READ_CHUNK_BYTES: usize = 8 * 1024;

pub(crate) type CaptureReader = Box<dyn AsyncRead + Unpin + Send>;

pub(crate) fn spawn(
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    writer: Arc<LogWriter>,
    state: Arc<ProcessCaptureState>,
    progress: Option<Arc<OutputTail>>,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
) -> ProcessCaptureTask {
    spawn_with_sink(
        stdout.map(|pipe| Box::new(pipe) as CaptureReader),
        stderr.map(|pipe| Box::new(pipe) as CaptureReader),
        Arc::new(LogCaptureSink::new(writer)),
        state,
        progress,
        sanitizer,
    )
}

pub(crate) fn spawn_with_sink(
    stdout: Option<CaptureReader>,
    stderr: Option<CaptureReader>,
    sink: Arc<dyn CaptureSink>,
    state: Arc<ProcessCaptureState>,
    progress: Option<Arc<OutputTail>>,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
) -> ProcessCaptureTask {
    let failed_state = state.clone();
    ProcessCaptureTask::spawn(
        async move {
            Coordinator::new(sink, state, progress, sanitizer)
                .run(stdout, stderr)
                .await
        },
        move || failed_state.record_capture_failure(),
    )
}

struct Coordinator {
    sink: Arc<dyn CaptureSink>,
    state: Arc<ProcessCaptureState>,
    progress: Option<Arc<OutputTail>>,
    stream: Option<Box<dyn ProcessOutputStream>>,
    committed: usize,
    framer: Framer,
    previews: ProcessPreviews,
}

impl Coordinator {
    fn new(
        sink: Arc<dyn CaptureSink>,
        state: Arc<ProcessCaptureState>,
        progress: Option<Arc<OutputTail>>,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Self {
        let progress_lines = progress.as_ref().map_or(0, |tail| tail.max_lines());
        Self {
            sink,
            state,
            progress,
            stream: sanitizer.map(|factory| factory.stream()),
            committed: 0,
            framer: Framer::new(),
            previews: ProcessPreviews::new(progress_lines),
        }
    }

    async fn run(
        mut self,
        mut stdout: Option<CaptureReader>,
        mut stderr: Option<CaptureReader>,
    ) -> Result<(), ToolIoError> {
        let mut stdout_decoder = Utf8Decoder::new();
        let mut stderr_decoder = Utf8Decoder::new();
        let mut stdout_buf = Zeroizing::new(vec![0; READ_CHUNK_BYTES]);
        let mut stderr_buf = Zeroizing::new(vec![0; READ_CHUNK_BYTES]);
        while stdout.is_some() || stderr.is_some() {
            tokio::select! {
                result = read_pipe(&mut stdout, &mut stdout_buf), if stdout.is_some() => {
                    let read = result?;
                    self.handle_read(CaptureSource::Stdout, read, &stdout_buf, &mut stdout_decoder).await?;
                    if read == 0 { stdout = None; }
                }
                result = read_pipe(&mut stderr, &mut stderr_buf), if stderr.is_some() => {
                    let read = result?;
                    self.handle_read(CaptureSource::Stderr, read, &stderr_buf, &mut stderr_decoder).await?;
                    if read == 0 { stderr = None; }
                }
            }
        }
        self.finish_stream().await
    }

    async fn handle_read(
        &mut self,
        source: CaptureSource,
        read: usize,
        buffer: &[u8],
        decoder: &mut Utf8Decoder,
    ) -> Result<(), ToolIoError> {
        if read == 0 {
            let final_bytes = decoder.finish();
            self.push(source, final_bytes).await?;
            self.framer.finish_source(source);
            self.publish_committed();
        } else {
            let decoded = decoder.push(&buffer[..read]);
            self.push(source, decoded).await?;
        }
        Ok(())
    }

    async fn push(
        &mut self,
        source: CaptureSource,
        bytes: Zeroizing<Vec<u8>>,
    ) -> Result<(), ToolIoError> {
        let framed = self.framer.frame(source, bytes);
        if framed.is_empty() {
            return Ok(());
        }
        let safe = if let Some(stream) = &mut self.stream {
            let safe = stream.sanitize(&framed);
            self.committed = stream.committed_input_bytes();
            safe
        } else {
            self.committed += framed.len();
            framed.to_vec()
        };
        persist(self.sink.as_ref(), &safe).await?;
        self.publish_committed();
        Ok(())
    }

    async fn finish_stream(&mut self) -> Result<(), ToolIoError> {
        let safe = if let Some(stream) = &mut self.stream {
            let safe = stream.finish();
            self.committed = stream.committed_input_bytes();
            safe
        } else {
            Vec::new()
        };
        persist(self.sink.as_ref(), &safe).await?;
        self.sink
            .flush()
            .await
            .map_err(|_| capture_error("final log flush failed"))?;
        self.publish_committed();
        Ok(())
    }

    fn publish_committed(&mut self) {
        let actions = self.framer.take_committed(self.committed);
        if actions.is_empty() {
            return;
        }
        for action in actions {
            match action {
                PreviewAction::Data(source, bytes) => self.previews.absorb(source, &bytes),
                PreviewAction::Finish(source) => self.previews.finish(source),
            }
        }
        let rendered = self.previews.render();
        let progress = self.state.publish(
            rendered.stdout,
            rendered.stdout_truncated,
            rendered.stderr,
            rendered.stderr_truncated,
            &rendered.progress,
        );
        if let Some(tail) = &self.progress {
            tail.replace_snapshot(progress);
        }
    }
}
