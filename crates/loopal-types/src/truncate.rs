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
