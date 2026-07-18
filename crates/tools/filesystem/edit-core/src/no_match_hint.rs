//! Actionable diagnostics for a failed exact-match edit. The text is fed back to
//! the model so it can fix the edit in one step instead of blindly retrying.

/// Explain why an exact `old_string` match failed against `content` and how to
/// fix it: a whitespace/line-ending difference when that alone is the cause, the
/// nearest line in the file, and a nudge to re-read possibly-stale content.
pub fn no_match_hint(content: &str, old_string: &str) -> String {
    let mut hint = String::new();

    if let Some(reason) = only_differs_by(content, old_string) {
        hint.push_str(reason);
        hint.push('\n');
    }

    if let Some((line_no, line)) = nearest_line(content, old_string) {
        hint.push_str(&format!("Nearest line is {line_no}: {}\n", clip(line)));
    }

    hint.push_str(
        "Re-read the file to copy the current exact text — including indentation — \
         since it may have changed since you last read it.",
    );
    hint
}

/// Name the single normalization that would make `old_string` match, when one
/// does — these are the fixes the model can apply without another read.
fn only_differs_by(content: &str, old_string: &str) -> Option<&'static str> {
    if old_string.is_empty() || content.contains(old_string) {
        return None;
    }
    let crlf_norm = |s: &str| s.replace("\r\n", "\n");
    if crlf_norm(content).contains(&crlf_norm(old_string)) {
        return Some(
            "The text matches except for line endings: the file uses CRLF (\\r\\n). \
             Copy the bytes from a fresh Read.",
        );
    }
    if collapse_ws(content).contains(&collapse_ws(old_string)) {
        return Some(
            "The text matches except for whitespace: check exact indentation \
             (tabs vs spaces) and blank lines.",
        );
    }
    None
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Find the file line most likely to be the intended target, anchored on the
/// longest distinctive token from the first non-blank line of `old_string`.
fn nearest_line(content: &str, old_string: &str) -> Option<(usize, String)> {
    let query = old_string.lines().find(|l| !l.trim().is_empty())?.trim();
    let token = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() >= 4)
        .max_by_key(|t| t.len())?;
    content
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(token))
        .map(|(i, line)| (i + 1, line.to_string()))
}

fn clip(line: String) -> String {
    const MAX: usize = 160;
    if line.chars().count() <= MAX {
        return line;
    }
    let mut clipped: String = line.chars().take(MAX).collect();
    clipped.push('…');
    clipped
}
