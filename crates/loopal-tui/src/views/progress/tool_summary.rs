/// Tool result summarization for single-line display.
///
/// Extracts metadata (line count, error first line, byte count) from
/// the full tool result string. The full result is preserved in
/// `DisplayToolCall.result` but NOT rendered.
use loopal_session::types::DisplayToolCall;

/// Produce a single-line summary for a tool call.
///
/// Format: `✓ Read(src/login.rs)  38 lines`
///         `✗ Bash(npm test)  Error: test failed`
///         `⋯ Edit(src/lib.rs:42)  running`
pub fn tool_call_summary(tc: &DisplayToolCall) -> (String, &'static str) {
    let icon = match tc.status.as_str() {
        "success" => "✓",
        "error" => "✗",
        _ => "⋯",
    };
    let detail = summarize_result(tc.result.as_deref(), &tc.status);
    let line = format!("  {} {}  {}", icon, tc.summary, detail);
    let color = match tc.status.as_str() {
        "success" => "green",
        "error" => "red",
        _ => "yellow",
    };
    (line, color)
}

/// Extract a short detail string from a tool result.
pub fn summarize_result(result: Option<&str>, status: &str) -> String {
    match result {
        None if status == "pending" || status == "running" => String::new(),
        None => String::new(),
        Some("") => "done".to_string(),
        Some(r) if status == "error" => {
            // First non-empty line of the error
            r.lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("error")
                .to_string()
        }
        Some(r) => {
            let count = r.lines().count();
            if count <= 1 {
                // Short result — show inline; long single line — truncate
                let trimmed = r.trim();
                if trimmed.len() <= 40 {
                    trimmed.to_string()
                } else {
                    let preview: String = trimmed.chars().take(37).collect();
                    format!("{preview}...")
                }
            } else {
                format!("{count} lines")
            }
        }
    }
}
