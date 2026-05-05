use crate::truncate::needs_truncation;

pub fn truncate_middle(output: &str, max_lines: usize, max_bytes: usize, head_ratio: u8) -> String {
    if !needs_truncation(output, max_lines, max_bytes) {
        return output.to_string();
    }
    let ratio = head_ratio.clamp(10, 90) as usize;
    let head_lines = (max_lines * ratio / 100).max(1);
    let tail_lines = max_lines.saturating_sub(head_lines).max(1);
    let head_bytes = (max_bytes * ratio / 100).max(1);
    let tail_bytes = max_bytes.saturating_sub(head_bytes).max(1);

    let mut head = String::new();
    let mut head_byte_used = 0;
    let mut head_lines_used = 0;
    let mut head_byte_end = 0;
    for line in output.lines().take(head_lines) {
        let lb = line.len() + 1;
        if head_byte_used + lb > head_bytes {
            break;
        }
        if head_lines_used > 0 {
            head.push('\n');
        }
        head.push_str(line);
        head_byte_used += lb;
        head_lines_used += 1;
        head_byte_end += lb;
    }

    let nth_from_end = tail_lines.saturating_sub(1);
    let tail_byte_start = output
        .rmatch_indices('\n')
        .nth(nth_from_end)
        .map(|(i, _)| i + 1)
        .unwrap_or(0)
        .max(head_byte_end);
    let mut tail = &output[tail_byte_start..];
    if tail.len() > tail_bytes {
        let mut cut = tail.len() - tail_bytes;
        while cut < tail.len() && !tail.is_char_boundary(cut) {
            cut += 1;
        }
        tail = if cut >= tail.len() {
            ""
        } else if let Some(adj) = tail[cut..].find('\n') {
            &tail[cut + adj + 1..]
        } else {
            &tail[cut..]
        };
    }
    let tail = tail.trim_end_matches('\n');

    let actual_tail_start = output.len().saturating_sub(tail.len());
    if head_byte_end >= actual_tail_start {
        return output.to_string();
    }

    let total_lines = output.lines().count();
    let tail_lines_used = tail.lines().count();
    let omitted_lines = total_lines.saturating_sub(head_lines_used + tail_lines_used);
    let omitted_bytes = output.len().saturating_sub(head.len() + tail.len());

    let prefix = if head.is_empty() {
        String::new()
    } else {
        format!("{head}\n")
    };
    format!(
        "{prefix}... [middle truncated: {omitted_lines} lines, {omitted_bytes} bytes omitted] ...\n{tail}"
    )
}
