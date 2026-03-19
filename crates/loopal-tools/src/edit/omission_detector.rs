/// Common patterns that LLMs use to skip code sections.
const OMISSION_PATTERNS: &[&str] = &[
    "// ... rest",
    "// ...rest",
    "// … rest",
    "// ... remaining",
    "// ...remaining",
    "# ... rest",
    "# ...rest",
    "# ... remaining",
    "/* ... */",
    "// ... existing code",
    "// ...existing code",
    "// ... keep",
    "// ... same as",
    "// ... unchanged",
    "// TODO: rest",
    "// [rest of",
    "// (rest of",
    "... (remaining",
    "... (rest of",
    "// ... other",
    "// ...other",
];

/// Check if text contains patterns that indicate an LLM omitted code.
///
/// Returns a list of detected omission patterns found in the text.
pub fn detect_omissions(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    OMISSION_PATTERNS
        .iter()
        .filter(|p| lower.contains(&p.to_lowercase()))
        .map(|p| p.to_string())
        .collect()
}