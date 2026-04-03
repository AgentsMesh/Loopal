/// Default maximum lines in tool output.
pub const DEFAULT_MAX_OUTPUT_LINES: usize = 2_000;
/// Default maximum bytes in tool output.
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 512_000;

/// Truncate tool output to fit within limits.
///
/// If the output exceeds `max_lines` or `max_bytes`, it is truncated
/// and a notice is appended indicating how much was omitted.
pub fn truncate_output(output: &str, max_lines: usize, max_bytes: usize) -> String {
    if output.is_empty() {
        return String::new();
    }

    let mut result = String::new();
    let mut byte_count = 0;
    let total_lines = output.lines().count();
    let total_bytes = output.len();

    for (line_count, line) in output.lines().enumerate() {
        let line_bytes = line.len() + 1; // +1 for newline
        if line_count >= max_lines || byte_count + line_bytes > max_bytes {
            let remaining_lines = total_lines - line_count;
            let remaining_bytes = total_bytes - byte_count;
            result.push_str(&format!(
                "\n... truncated ({remaining_lines} lines, {remaining_bytes} bytes omitted)"
            ));
            return result;
        }
        if line_count > 0 {
            result.push('\n');
        }
        result.push_str(line);
        byte_count += line_bytes;
    }

    result
}

/// Check whether the output would be truncated at the given limits.
pub fn needs_truncation(output: &str, max_lines: usize, max_bytes: usize) -> bool {
    output.len() > max_bytes || output.lines().count() > max_lines
}

/// Result of overflow handling.
pub struct OverflowResult {
    /// Content to return to LLM (full output or preview + file path).
    pub display: String,
    /// Whether the output was saved to a file.
    pub overflowed: bool,
}

/// Handle oversized output: save full content to a file, return preview + path.
///
/// If the output fits within limits, returns it unchanged. Otherwise, saves the
/// complete output to `{tmp}/loopal/overflow/{label}_{timestamp}.txt` and returns
/// a truncated preview with a reference to the file. The LLM can use the Read
/// tool to access the full content on demand.
pub fn handle_overflow(
    output: &str,
    max_lines: usize,
    max_bytes: usize,
    label: &str,
) -> OverflowResult {
    if !needs_truncation(output, max_lines, max_bytes) {
        return OverflowResult {
            display: output.to_string(),
            overflowed: false,
        };
    }

    let path = save_to_overflow_file(output, label);
    // Show a preview (25% of limits) so LLM has context before reading the file.
    let preview_lines = max_lines / 4;
    let preview_bytes = max_bytes / 4;
    let preview = truncate_output(output, preview_lines, preview_bytes);
    let total = humanize_size(output.len());

    let display = format!(
        "{preview}\n\n\
         [Output too large for context ({total}). Full output saved to: {path}]\n\
         Use the Read tool to access the complete output if needed."
    );
    OverflowResult {
        display,
        overflowed: true,
    }
}

/// Save content to an overflow file. Returns the absolute path.
pub fn save_to_overflow_file(content: &str, label: &str) -> String {
    let dir = overflow_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return "(failed to save overflow file)".into();
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = dir.join(format!("{label}_{ts}.txt"));
    match std::fs::write(&path, content) {
        Ok(()) => path.to_string_lossy().into_owned(),
        Err(_) => "(failed to save overflow file)".into(),
    }
}

/// Overflow file directory: {tmp}/loopal/overflow/
fn overflow_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("loopal").join("overflow")
}

fn humanize_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
