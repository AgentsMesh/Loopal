use std::path::{Path, PathBuf};

use super::omission_detector::detect_omissions;
use super::patch_types::{FileOp, FileWrite, Hunk, HunkLine};

#[derive(Debug)]
pub struct PatchApplyError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for PatchApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

/// Apply parsed file operations, returning write instructions.
///
/// `read_file` is injected for testability (avoids real FS in unit tests).
pub fn apply_file_ops(
    ops: &[FileOp],
    cwd: &Path,
    read_file: impl Fn(&Path) -> std::io::Result<String>,
) -> Result<Vec<FileWrite>, PatchApplyError> {
    let mut writes = Vec::new();
    for op in ops {
        match op {
            FileOp::Add { path, content } => {
                let full = resolve(path, cwd);
                if full.exists() {
                    return Err(err(path, "file already exists"));
                }
                check_omissions(path, content)?;
                writes.push(FileWrite { path: full, content: Some(content.clone()) });
            }
            FileOp::Delete { path } => {
                let full = resolve(path, cwd);
                if !full.exists() {
                    return Err(err(path, "file does not exist"));
                }
                writes.push(FileWrite { path: full, content: None });
            }
            FileOp::Update { path, hunks } => {
                let full = resolve(path, cwd);
                let original = read_file(&full)
                    .map_err(|e| err(path, &format!("cannot read: {e}")))?;
                let updated = apply_hunks(path, &original, hunks)?;
                writes.push(FileWrite { path: full, content: Some(updated) });
            }
        }
    }
    Ok(writes)
}

fn apply_hunks(path: &Path, original: &str, hunks: &[Hunk]) -> Result<String, PatchApplyError> {
    let mut lines: Vec<String> = original.lines().map(String::from).collect();

    // Find all match positions first, then apply bottom-up
    let mut matches: Vec<(usize, usize, Vec<String>)> = Vec::new();
    for hunk in hunks {
        let search = search_lines(hunk);
        let pos = find_match(&lines, &search, hunk.line_hint).ok_or_else(|| {
            let preview: Vec<_> = search.iter().take(3).map(|s| s.as_str()).collect();
            err(path, &format!("hunk not found, expected: {preview:?}"))
        })?;
        let output = output_lines(hunk);
        check_omissions_lines(path, &output)?;
        matches.push((pos, search.len(), output));
    }

    // Sort descending by position to avoid line-number shifts
    matches.sort_by(|a, b| b.0.cmp(&a.0));
    for (pos, search_len, output) in matches {
        lines.splice(pos..pos + search_len, output);
    }

    let mut result = lines.join("\n");
    if original.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

/// Context + Remove lines = the pattern to locate in the file.
fn search_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines.iter().filter_map(|l| match l {
        HunkLine::Context(s) | HunkLine::Remove(s) => Some(s.clone()),
        HunkLine::Add(_) => None,
    }).collect()
}

/// Context + Add lines = the replacement content.
fn output_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines.iter().filter_map(|l| match l {
        HunkLine::Context(s) | HunkLine::Add(s) => Some(s.clone()),
        HunkLine::Remove(_) => None,
    }).collect()
}

fn find_match(file_lines: &[String], search: &[String], hint: Option<usize>) -> Option<usize> {
    if search.is_empty() {
        return None;
    }
    // Pass 1: exact match
    let exact: Vec<usize> = (0..=file_lines.len().saturating_sub(search.len()))
        .filter(|&i| file_lines[i..i + search.len()].iter().zip(search).all(|(a, b)| a == b))
        .collect();
    if let Some(pos) = disambiguate(&exact, hint) {
        return Some(pos);
    }
    // Pass 2: trim-whitespace fallback
    let trimmed: Vec<usize> = (0..=file_lines.len().saturating_sub(search.len()))
        .filter(|&i| {
            file_lines[i..i + search.len()]
                .iter()
                .zip(search)
                .all(|(a, b)| a.trim() == b.trim())
        })
        .collect();
    disambiguate(&trimmed, hint)
}

fn disambiguate(positions: &[usize], hint: Option<usize>) -> Option<usize> {
    match positions.len() {
        0 => None,
        1 => Some(positions[0]),
        _ => {
            let target = hint?.saturating_sub(1); // 1-based → 0-based
            Some(*positions.iter().min_by_key(|&&p| p.abs_diff(target)).unwrap())
        }
    }
}

fn check_omissions(path: &Path, content: &str) -> Result<(), PatchApplyError> {
    let om = detect_omissions(content);
    if !om.is_empty() {
        return Err(err(path, &format!("omission detected: {}", om.join(", "))));
    }
    Ok(())
}

fn check_omissions_lines(path: &Path, lines: &[String]) -> Result<(), PatchApplyError> {
    for line in lines {
        let om = detect_omissions(line);
        if !om.is_empty() {
            return Err(err(path, &format!("omission detected: {}", om.join(", "))));
        }
    }
    Ok(())
}

fn resolve(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) }
}

fn err(path: &Path, message: &str) -> PatchApplyError {
    PatchApplyError { path: path.to_path_buf(), message: message.to_string() }
}
