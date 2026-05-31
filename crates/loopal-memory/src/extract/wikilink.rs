use std::sync::LazyLock;

use regex::Regex;

static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([a-z][a-z0-9_-]*)(?:\|[^\]]+)?\]\]").unwrap());

pub struct WikiLink {
    pub slug: String,
    pub line: u32,
}

pub fn scan(body: &str) -> Vec<WikiLink> {
    let mut out = Vec::new();
    for (line_idx, line_text) in body.lines().enumerate() {
        for cap in WIKILINK_RE.captures_iter(line_text) {
            if let Some(slug) = cap.get(1) {
                out.push(WikiLink {
                    slug: slug.as_str().to_string(),
                    line: (line_idx + 1) as u32,
                });
            }
        }
    }
    out
}

pub fn normalize_to_slug(reference: &str) -> Option<String> {
    let s = reference.trim();
    if s.is_empty() {
        return None;
    }
    let s = s.trim_start_matches("[[").trim_end_matches("]]");
    let s = s.split('|').next().unwrap_or(s);
    let s = s.trim_end_matches(".md");
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if !s.starts_with(|c: char| c.is_ascii_lowercase()) {
        return None;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return None;
    }
    Some(s.to_string())
}
