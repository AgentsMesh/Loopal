//! Condition expression for hook matching.
//!
//! Replaces simple `tool_filter: ["Bash"]` with expressive patterns:
//! - `"Bash(git push*)"` — tool name + argument glob
//! - `"Write(*.rs)"` — file path glob
//! - `"Bash|Write"` — OR multiple tools
//! - `"*"` — match everything

/// Parsed condition for hook matching.
///
/// Stored as a raw string in config; parsed on demand for matching.
/// This avoids upfront compilation cost for hooks that may never fire.
pub fn matches_condition(condition: &str, tool_name: &str, tool_input: &serde_json::Value) -> bool {
    if condition == "*" {
        return true;
    }

    // OR syntax: "Bash|Write|Edit"
    if condition.contains('|') && !condition.contains('(') {
        return condition.split('|').any(|part| part.trim() == tool_name);
    }

    // Tool(glob) syntax: "Bash(git push*)"
    if let Some(paren_start) = condition.find('(') {
        let name_part = &condition[..paren_start];
        if name_part != tool_name {
            return false;
        }
        let glob_part = condition[paren_start + 1..]
            .strip_suffix(')')
            .unwrap_or(&condition[paren_start + 1..]);
        let primary_arg = extract_primary_arg(tool_name, tool_input);
        return glob_match(glob_part, &primary_arg);
    }

    // Plain tool name: "Bash"
    condition == tool_name
}

/// Extract the "primary argument" from tool input for glob matching.
fn extract_primary_arg(tool_name: &str, input: &serde_json::Value) -> String {
    let field = match tool_name {
        "Bash" => "command",
        "Write" | "Edit" | "MultiEdit" | "Read" => "file_path",
        "ApplyPatch" => "patch",
        "Grep" | "Glob" => "pattern",
        "Fetch" => "url",
        "WebSearch" => "query",
        "Ls" => "path",
        _ => return input.to_string(),
    };
    input
        .get(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Simple glob matching: `*` matches any substring, `?` matches one char.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_match_inner(&p, &t, 0, 0)
}

fn glob_match_inner(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }
    if pattern[pi] == '*' {
        // '*' matches zero or more characters
        for skip in ti..=text.len() {
            if glob_match_inner(pattern, text, pi + 1, skip) {
                return true;
            }
        }
        return false;
    }
    if ti == text.len() {
        return false;
    }
    if pattern[pi] == '?' || pattern[pi] == text[ti] {
        return glob_match_inner(pattern, text, pi + 1, ti + 1);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wildcard_matches_all() {
        assert!(matches_condition("*", "Bash", &json!({})));
        assert!(matches_condition("*", "Write", &json!({})));
    }

    #[test]
    fn plain_tool_name() {
        assert!(matches_condition("Bash", "Bash", &json!({})));
        assert!(!matches_condition("Bash", "Write", &json!({})));
    }

    #[test]
    fn or_syntax() {
        assert!(matches_condition("Bash|Write", "Bash", &json!({})));
        assert!(matches_condition("Bash|Write", "Write", &json!({})));
        assert!(!matches_condition("Bash|Write", "Read", &json!({})));
    }

    #[test]
    fn tool_with_glob() {
        let input = json!({"command": "git push origin main"});
        assert!(matches_condition("Bash(git push*)", "Bash", &input));
        assert!(!matches_condition("Bash(git pull*)", "Bash", &input));
    }

    #[test]
    fn file_path_glob() {
        let input = json!({"file_path": "src/main.rs"});
        assert!(matches_condition("Write(*.rs)", "Write", &input));
        assert!(!matches_condition("Write(*.ts)", "Write", &input));
    }

    #[test]
    fn wrong_tool_with_glob() {
        let input = json!({"command": "git push"});
        assert!(!matches_condition("Write(git*)", "Bash", &input));
    }
}
