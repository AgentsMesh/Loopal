use serde::Deserialize;

use crate::extract::errors::ExtractionError;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub ttl_days: Option<u32>,
    #[serde(default)]
    pub related: Vec<String>,
}

pub struct ParsedFile {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub errors: Vec<ExtractionError>,
}

pub fn parse(text: &str) -> ParsedFile {
    let mut errors = Vec::new();

    if has_merge_conflict_marker(text) {
        errors.push(ExtractionError::MergeConflictMarker);
    }

    let (raw_yaml, body) = split_frontmatter(text);
    let body = body.to_string();

    let frontmatter = match raw_yaml {
        Some(yaml) => match serde_yaml::from_str::<Frontmatter>(yaml) {
            Ok(fm) => fm,
            Err(e) => {
                errors.push(ExtractionError::FrontmatterParse(e.to_string()));
                Frontmatter::default()
            }
        },
        None => Frontmatter::default(),
    };

    ParsedFile {
        frontmatter,
        body,
        errors,
    }
}

fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let trimmed = text.trim_start_matches('\u{feff}');
    if !trimmed.starts_with("---") {
        return (None, text);
    }
    let after_first = match trimmed.strip_prefix("---") {
        Some(s) => s,
        None => return (None, text),
    };
    let after_first = strip_line_terminator(after_first);

    if let Some(end_idx) = find_yaml_terminator(after_first) {
        let yaml = &after_first[..end_idx];
        let rest_start = end_idx + 3;
        let rest = &after_first[rest_start..];
        let body = strip_line_terminator(rest);
        (Some(yaml), body)
    } else {
        (None, text)
    }
}

fn strip_line_terminator(s: &str) -> &str {
    s.strip_prefix("\r\n")
        .or_else(|| s.strip_prefix('\n'))
        .unwrap_or(s)
}

fn find_yaml_terminator(s: &str) -> Option<usize> {
    let mut start = 0;
    for line in s.split_inclusive('\n') {
        let stripped = line.trim_end();
        if stripped == "---" {
            return Some(start);
        }
        start += line.len();
    }
    None
}

fn has_merge_conflict_marker(text: &str) -> bool {
    // reason: `=======` 也是 markdown setext H1 underline；只有当 <<<<<<< 和 >>>>>>>
    //  两端 marker 都存在才认定为真正的 git merge conflict。
    let mut has_start = false;
    let mut has_end = false;
    for line in text.lines() {
        if line.starts_with("<<<<<<<") {
            has_start = true;
        }
        if line.starts_with(">>>>>>>") {
            has_end = true;
        }
    }
    has_start && has_end
}
