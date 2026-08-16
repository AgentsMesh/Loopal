use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;

use loopal_tool_api::ProcessOutputSanitizer;
use zeroize::Zeroizing;

use crate::process_capture_state::ProcessCompletion;

pub(crate) fn guard(sanitizer: &Option<Arc<dyn ProcessOutputSanitizer>>, text: &str) -> String {
    sanitizer
        .as_ref()
        .map_or_else(|| text.to_string(), |guard| guard.guard_text(text))
}

pub(crate) fn guard_with_suffix(
    sanitizer: &Option<Arc<dyn ProcessOutputSanitizer>>,
    text: &str,
    suffix: &str,
) -> String {
    let mut composite = Zeroizing::new(String::with_capacity(text.len() + suffix.len()));
    composite.push_str(text);
    composite.push_str(suffix);
    guard(sanitizer, &composite)
}

pub(crate) fn status_text(completion: ProcessCompletion, exit_code: Option<i32>) -> String {
    match (completion, exit_code) {
        (ProcessCompletion::Completed, Some(code)) => format!("[Completed, exit {code}]"),
        (ProcessCompletion::Completed, None) => "[Status: Completed]".into(),
        (ProcessCompletion::Failed, Some(code)) => format!("[Failed, exit {code}]"),
        (ProcessCompletion::Failed, None) => "[Status: Failed]".into(),
        (ProcessCompletion::Killed, Some(code)) => format!("[Killed, exit {code}]"),
        (ProcessCompletion::Killed, None) => "[Status: Killed]".into(),
    }
}

pub(crate) fn render_preview(
    stdout: &str,
    stdout_truncated: bool,
    stderr: &str,
    stderr_truncated: bool,
    log_path: &Path,
) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::new());
    if stdout_truncated {
        out.push_str("[stdout, truncated]\n");
    } else {
        out.push_str("[stdout]\n");
    }
    out.push_str(stdout);
    if !stderr.is_empty() {
        if stderr_truncated {
            out.push_str("\n\n[stderr, truncated to last 8 KB]\n");
        } else {
            out.push_str("\n\n[stderr]\n");
        }
        out.push_str(stderr);
    }
    out.push_str("\n\n[full log: ");
    let _ = write!(&mut *out, "{}", log_path.display());
    out.push(']');
    out
}
