pub const DEFAULT_MAX_OUTPUT_LINES: usize = 2_000;
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 512_000;

pub fn truncate_output(output: &str, max_lines: usize, max_bytes: usize) -> String {
    if output.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    let mut byte_count = 0;
    let total_lines = output.lines().count();
    let total_bytes = output.len();
    for (line_count, line) in output.lines().enumerate() {
        let line_bytes = line.len() + 1;
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

pub fn needs_truncation(output: &str, max_lines: usize, max_bytes: usize) -> bool {
    output.len() > max_bytes || output.lines().count() > max_lines
}

pub fn truncate_tail(output: &str, max_lines: usize, max_bytes: usize) -> String {
    if !needs_truncation(output, max_lines, max_bytes) {
        return output.to_string();
    }
    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();
    let total_bytes = output.len();
    let mut tail_collected: Vec<&str> = Vec::new();
    let mut byte_used = 0;
    for line in lines.iter().rev() {
        let lb = line.len() + 1;
        if tail_collected.len() >= max_lines || byte_used + lb > max_bytes {
            break;
        }
        tail_collected.push(line);
        byte_used += lb;
    }
    tail_collected.reverse();
    let tail = tail_collected.join("\n");
    let kept_lines = tail_collected.len();
    let omitted_lines = total_lines - kept_lines;
    let omitted_bytes = total_bytes.saturating_sub(byte_used);
    format!("[head truncated: {omitted_lines} lines, {omitted_bytes} bytes omitted]\n{tail}")
}

pub fn extract_overflow_path(s: &str) -> (String, Option<String>) {
    const MARKER: &str = "[Output too large for context (";
    const PATH_PREFIX: &str = "Full output saved to: ";
    const TAIL: &str = "Use the Read tool to access the complete output if needed.";
    if !s.ends_with(TAIL) {
        return (s.to_string(), None);
    }
    let Some(marker_idx) = s.rfind(MARKER) else {
        return (s.to_string(), None);
    };
    let after_marker = &s[marker_idx..];
    let Some(path_pre_rel) = after_marker.find(PATH_PREFIX) else {
        return (s.to_string(), None);
    };
    let path_start = marker_idx + path_pre_rel + PATH_PREFIX.len();
    let Some(rel_close) = s[path_start..].find(']') else {
        return (s.to_string(), None);
    };
    let path = s[path_start..path_start + rel_close].to_string();
    let body = s[..marker_idx].trim_end_matches('\n').to_string();
    (body, Some(path))
}

pub struct OverflowResult {
    pub display: String,
    pub overflowed: bool,
}

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

fn overflow_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("loopal").join("overflow")
}

pub fn humanize_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
