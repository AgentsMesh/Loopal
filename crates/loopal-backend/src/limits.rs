use std::time::Duration;

use loopal_tool_api::{DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES};

pub const IMAGE_MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const IMAGE_MAX_PIXELS: u64 = 8192 * 8192;

/// Resource limits applied by `LocalBackend`.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum file size in bytes that `read()` will accept.
    pub max_file_read_bytes: u64,
    /// Maximum lines in command output.
    pub max_output_lines: usize,
    /// Maximum bytes in command output.
    pub max_output_bytes: usize,
    /// Cap on glob result count before truncation.
    pub max_glob_results: usize,
    /// Cap on grep match count before truncation.
    pub max_grep_matches: usize,
    /// Maximum HTTP response body size in bytes.
    pub max_fetch_bytes: usize,
    /// Default shell command timeout.
    pub default_timeout: Duration,
    /// Cooperative deadline checked between glob/grep walk entries: bounds
    /// slow-but-responsive trees and returns partial results. A syscall stuck
    /// on a dead mount can't be interrupted here — that case is bounded by the
    /// runtime per-tool watchdog instead.
    pub walk_timeout: Duration,
    /// HTTP fetch timeout.
    pub fetch_timeout: Duration,
    /// Maximum image file size in bytes.
    pub image_max_bytes: u64,
    /// Maximum image pixels (width × height).
    pub image_max_pixels: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_read_bytes: 10 * 1024 * 1024, // 10 MB
            max_output_lines: DEFAULT_MAX_OUTPUT_LINES,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            max_glob_results: 10_000,
            max_grep_matches: 500,
            max_fetch_bytes: 5 * 1024 * 1024,          // 5 MB
            default_timeout: Duration::from_secs(300), // 5 min
            walk_timeout: Duration::from_secs(30),
            fetch_timeout: Duration::from_secs(30),
            image_max_bytes: IMAGE_MAX_BYTES,
            image_max_pixels: IMAGE_MAX_PIXELS,
        }
    }
}
