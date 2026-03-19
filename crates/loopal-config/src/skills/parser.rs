/// A loaded skill definition parsed from a `.md` file.
#[derive(Debug, Clone)]
pub struct Skill {
    /// Command name including the leading `/`, e.g. "/commit"
    pub name: String,
    /// Short description (from frontmatter or first line of body)
    pub description: String,
    /// Whether the body contains the `$ARGUMENTS` placeholder
    pub has_arg: bool,
    /// Prompt template body (everything after the frontmatter)
    pub body: String,
}

/// Parse a single `.md` skill file content into a `Skill`.
///
/// `name` should already include the leading `/`, e.g. "/commit".
pub fn parse_skill(name: &str, content: &str) -> Skill {
    let (description, body) = parse_frontmatter(content);

    let description = description.unwrap_or_else(|| {
        // Fall back to first non-empty line of body, truncated to 60 chars
        let first = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        truncate(first.trim(), 60).to_string()
    });

    let has_arg = body.contains("$ARGUMENTS");

    Skill { name: name.to_string(), description, has_arg, body }
}

/// Extract optional frontmatter `description` and the remaining body.
///
/// Frontmatter is a YAML-like block delimited by `---` lines at the start.
/// Only the `description` key is recognized; other keys are silently ignored.
fn parse_frontmatter(content: &str) -> (Option<String>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // Skip the opening "---" line
    let Some(first_newline) = trimmed.find('\n') else {
        return (None, content.to_string());
    };
    let after_open = &trimmed[first_newline + 1..];

    // Find the closing "---" — could be at start or after a newline
    let (fm_block, body) = if let Some(rest) = after_open.strip_prefix("---") {
        ("", rest.strip_prefix('\n').unwrap_or(rest))
    } else if let Some(end) = after_open.find("\n---") {
        let rest = &after_open[end + 4..];
        (&after_open[..end], rest.strip_prefix('\n').unwrap_or(rest))
    } else {
        // No closing delimiter — treat entire content as body
        return (None, content.to_string());
    };

    let mut description = None;
    for line in fm_block.lines() {
        if let Some(value) = line.trim().strip_prefix("description:") {
            description = Some(value.trim().to_string());
        }
    }

    (description, body.to_string())
}

/// Truncate a string to `max` characters, appending "…" if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}…")
    }
}
