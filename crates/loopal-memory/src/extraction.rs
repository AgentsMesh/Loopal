//! Utilities for extracting structured sections from agent output.

/// Extract memory suggestions from agent output.
///
/// Looks for ALL `## Memory Suggestions` sections and extracts bullet points.
/// Multiple sections are supported (e.g., if an agent produces them in separate parts).
pub fn extract_memory_suggestions(output: &str) -> Vec<String> {
    let mut suggestions = Vec::new();
    let mut in_section = false;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed == "## Memory Suggestions" {
            in_section = true;
            continue;
        }
        // Pause at a different heading (but allow re-entering on next ## Memory Suggestions)
        if in_section && trimmed.starts_with("## ") {
            in_section = false;
            continue;
        }
        if in_section && let Some(bullet) = trimmed.strip_prefix("- ") {
            let bullet = bullet.trim();
            if !bullet.is_empty() {
                suggestions.push(bullet.to_string());
            }
        }
    }
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_basic() {
        let output = "\
## Summary
Did some work.

## Memory Suggestions
- Use Redis for caching because Memcached lacks pub/sub
- Team prefers snake_case for all Rust modules

## Other
Done.";
        let suggestions = extract_memory_suggestions(output);
        assert_eq!(suggestions.len(), 2);
        assert_eq!(
            suggestions[0],
            "Use Redis for caching because Memcached lacks pub/sub"
        );
        assert_eq!(
            suggestions[1],
            "Team prefers snake_case for all Rust modules"
        );
    }

    #[test]
    fn test_extract_not_found() {
        let output = "## Summary\nDid some work.\nNo memory section here.";
        assert!(extract_memory_suggestions(output).is_empty());
    }

    #[test]
    fn test_extract_empty_bullets_skipped() {
        let output = "\
## Memory Suggestions
- Valid observation
-
- Another valid one
";
        let suggestions = extract_memory_suggestions(output);
        assert_eq!(suggestions, vec!["Valid observation", "Another valid one"]);
    }

    #[test]
    fn test_extract_stops_at_next_heading() {
        let output = "\
## Memory Suggestions
- First suggestion
## Next Section
- This should not be extracted
";
        let suggestions = extract_memory_suggestions(output);
        assert_eq!(suggestions, vec!["First suggestion"]);
    }

    #[test]
    fn test_extract_at_end_of_output() {
        let output = "\
Some text.
## Memory Suggestions
- Last observation";
        assert_eq!(extract_memory_suggestions(output), vec!["Last observation"]);
    }

    #[test]
    fn test_extract_ignores_non_bullet_lines() {
        let output = "\
## Memory Suggestions
Some intro text that is not a bullet.
- Actual suggestion
More non-bullet text.
- Second suggestion
";
        let suggestions = extract_memory_suggestions(output);
        assert_eq!(suggestions, vec!["Actual suggestion", "Second suggestion"]);
    }

    #[test]
    fn test_extract_multiple_sections() {
        let output = "\
## Analysis
Here is an example:

## Memory Suggestions
- Example entry from analysis

## Details
More explanation here.

## Memory Suggestions
- Actual finding: Redis chosen for pub/sub support
- Another real insight
";
        let suggestions = extract_memory_suggestions(output);
        assert_eq!(suggestions.len(), 3);
        assert_eq!(suggestions[0], "Example entry from analysis");
        assert_eq!(
            suggestions[1],
            "Actual finding: Redis chosen for pub/sub support"
        );
        assert_eq!(suggestions[2], "Another real insight");
    }

    #[test]
    fn test_extract_empty_output() {
        assert!(extract_memory_suggestions("").is_empty());
    }
}
