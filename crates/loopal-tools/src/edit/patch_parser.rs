use std::path::PathBuf;

use super::patch_types::{FileOp, Hunk, HunkLine};

#[derive(Debug)]
pub struct PatchParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for PatchParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// Parse a patch text into a list of file operations.
pub fn parse_patch(input: &str) -> Result<Vec<FileOp>, PatchParseError> {
    let mut ops = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            let (content, next) = parse_add_body(&lines, i + 1);
            ops.push(FileOp::Add { path: PathBuf::from(path.trim()), content });
            i = next;
        } else if let Some(path) = line.strip_prefix("*** Update File: ") {
            let (hunks, next) = parse_update_body(&lines, i + 1)?;
            if hunks.is_empty() {
                return Err(PatchParseError {
                    line: i + 1,
                    message: "update has no hunks".into(),
                });
            }
            ops.push(FileOp::Update { path: PathBuf::from(path.trim()), hunks });
            i = next;
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            ops.push(FileOp::Delete { path: PathBuf::from(path.trim()) });
            i += 1;
        } else if line.trim().is_empty() {
            i += 1;
        } else {
            return Err(PatchParseError {
                line: i + 1,
                message: format!("unexpected line: {line}"),
            });
        }
    }
    Ok(ops)
}

fn parse_add_body(lines: &[&str], start: usize) -> (String, usize) {
    let mut content_lines = Vec::new();
    let mut i = start;
    while i < lines.len() {
        if lines[i].starts_with("*** ") {
            break;
        }
        if let Some(rest) = lines[i].strip_prefix('+') {
            content_lines.push(rest);
        }
        i += 1;
    }
    let mut content = content_lines.join("\n");
    if !content_lines.is_empty() {
        content.push('\n');
    }
    (content, i)
}

fn parse_update_body(
    lines: &[&str],
    start: usize,
) -> Result<(Vec<Hunk>, usize), PatchParseError> {
    let mut hunks = Vec::new();
    let mut i = start;
    while i < lines.len() {
        if lines[i].starts_with("*** ") {
            break;
        }
        if lines[i].starts_with("@@") {
            let line_hint = parse_line_hint(lines[i]);
            let (hunk_lines, next) = parse_hunk_lines(lines, i + 1)?;
            hunks.push(Hunk { line_hint, lines: hunk_lines });
            i = next;
        } else if lines[i].trim().is_empty() {
            i += 1;
        } else {
            return Err(PatchParseError {
                line: i + 1,
                message: format!("expected @@ or file header, got: {}", lines[i]),
            });
        }
    }
    Ok((hunks, i))
}

fn parse_line_hint(line: &str) -> Option<usize> {
    let trimmed = line.trim().trim_start_matches('@').trim_end_matches('@').trim();
    if trimmed.is_empty() { None } else { trimmed.parse().ok() }
}

fn parse_hunk_lines(
    lines: &[&str],
    start: usize,
) -> Result<(Vec<HunkLine>, usize), PatchParseError> {
    let mut hunk_lines = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("@@") || line.starts_with("*** ") {
            break;
        }
        if let Some(rest) = line.strip_prefix('-') {
            hunk_lines.push(HunkLine::Remove(rest.to_string()));
        } else if let Some(rest) = line.strip_prefix('+') {
            hunk_lines.push(HunkLine::Add(rest.to_string()));
        } else if let Some(rest) = line.strip_prefix(' ') {
            hunk_lines.push(HunkLine::Context(rest.to_string()));
        } else if line.is_empty() {
            // Look ahead: if the next non-blank line is a file header or EOF,
            // this blank line is a separator, not content.
            let next_content = lines[i + 1..].iter().find(|l| !l.is_empty());
            let is_separator = next_content.is_none()
                || next_content.is_some_and(|n| n.starts_with("*** ") || n.starts_with("@@"));
            if is_separator {
                break;
            }
            hunk_lines.push(HunkLine::Context(String::new()));
        } else {
            return Err(PatchParseError {
                line: i + 1,
                message: format!("invalid hunk line prefix: {line}"),
            });
        }
        i += 1;
    }
    Ok((hunk_lines, i))
}
