/// Format a thinking summary for display.
pub fn format_thinking_summary(thinking: &str, token_count: u32) -> String {
    let token_display = if token_count >= 1000 {
        format!("{:.1}k", token_count as f64 / 1000.0)
    } else {
        format!("{token_count}")
    };
    // Take first line as preview
    let preview = thinking
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect::<String>();
    format!("[{token_display} tokens] {preview}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_large_token_count() {
        let result = format_thinking_summary("Hello world\nsecond line", 1500);
        assert!(result.contains("1.5k tokens"));
        assert!(result.contains("Hello world"));
    }

    #[test]
    fn format_small_token_count() {
        let result = format_thinking_summary("Short", 500);
        assert!(result.contains("500 tokens"));
    }
}
