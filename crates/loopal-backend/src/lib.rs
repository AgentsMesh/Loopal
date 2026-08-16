pub mod approved;
pub mod batch;
pub mod fs;
#[cfg(windows)]
mod fs_replace;
mod fs_write;
pub mod image;
pub mod limits;
pub mod local;
mod local_backend_impl;
mod local_process;
mod log_capture;
pub mod log_writer;
pub mod net;
pub mod path;
pub mod platform;
mod private_log_file;
mod process_capture;
mod process_capture_buffers;
mod process_capture_frame;
mod process_capture_io;
mod process_capture_preview;
mod process_capture_render;
mod process_capture_result;
mod process_capture_state;
pub mod process_capture_task;
mod process_cleanup;
pub mod process_group;
mod process_wait;
pub mod search;
pub mod shell;
pub(crate) mod shell_spawn;
pub mod shell_stream;
pub mod tmp_cleanup;

#[cfg(test)]
mod process_capture_tests;

pub use limits::ResourceLimits;
pub use local::LocalBackend;
pub use log_writer::{
    LineSink, LogWriter, create_log_file, create_log_file_in, flush_writer, read_lines_into_sink,
    session_bash_dir, session_bash_dir_in,
};
pub use process_capture_state::{ProcessCaptureSnapshot, ProcessCaptureState, ProcessCompletion};
pub use process_capture_task::ProcessCaptureTask;
pub use process_group::{KillOutcome, SpawnedChild, Termination};
pub use tmp_cleanup::{
    ORPHAN_MIN_AGE, cleanup_orphans, cleanup_orphans_in, cleanup_session_tmp,
    cleanup_session_tmp_in, loopal_tmp_root, session_tmp_root, session_tmp_root_in,
};
