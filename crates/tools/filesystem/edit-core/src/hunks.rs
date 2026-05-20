use crate::omission_detector::detect_omissions;
use crate::patch_types::{Hunk, HunkLine};

#[derive(Debug)]
pub enum HunkError {
    NotFound { preview: Vec<String> },
    Omission(Vec<String>),
    Overlapping { line_a: usize, line_b: usize },
}

impl std::fmt::Display for HunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { preview } => {
                let refs: Vec<&str> = preview.iter().map(|s| s.as_str()).collect();
                write!(f, "hunk not found, expected: {refs:?}")
            }
            Self::Omission(om) => write!(f, "omission detected: {}", om.join(", ")),
            Self::Overlapping { line_a, line_b } => write!(
                f,
                "overlapping hunks at line {} and line {}",
                line_a + 1,
                line_b + 1
            ),
        }
    }
}

pub fn apply_hunks_to_text(original: &str, hunks: &[Hunk]) -> Result<String, HunkError> {
    let mut lines: Vec<String> = original.lines().map(String::from).collect();

    let mut matches: Vec<(usize, usize, Vec<String>)> = Vec::new();
    for hunk in hunks {
        let search = search_lines(hunk);
        let pos = find_match(&lines, &search, hunk.line_hint).ok_or_else(|| {
            let preview: Vec<String> = search.iter().take(3).cloned().collect();
            HunkError::NotFound { preview }
        })?;
        let output = output_lines(hunk);
        let om = collect_omissions(&output);
        if !om.is_empty() {
            return Err(HunkError::Omission(om));
        }
        matches.push((pos, search.len(), output));
    }

    if let Some((a, b)) = first_overlap(&matches) {
        return Err(HunkError::Overlapping {
            line_a: a,
            line_b: b,
        });
    }

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

fn first_overlap(matches: &[(usize, usize, Vec<String>)]) -> Option<(usize, usize)> {
    let mut sorted: Vec<(usize, usize)> = matches.iter().map(|m| (m.0, m.1)).collect();
    sorted.sort_by_key(|t| t.0);
    for w in sorted.windows(2) {
        if w[0].0 + w[0].1 > w[1].0 {
            return Some((w[0].0, w[1].0));
        }
    }
    None
}

fn search_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) | HunkLine::Remove(s) => Some(s.clone()),
            HunkLine::Add(_) => None,
        })
        .collect()
}

fn output_lines(hunk: &Hunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) | HunkLine::Add(s) => Some(s.clone()),
            HunkLine::Remove(_) => None,
        })
        .collect()
}

fn collect_omissions(lines: &[String]) -> Vec<String> {
    let mut all = Vec::new();
    for line in lines {
        all.extend(detect_omissions(line));
    }
    all
}

fn find_match(file_lines: &[String], search: &[String], hint: Option<usize>) -> Option<usize> {
    if search.is_empty() {
        return None;
    }
    let exact: Vec<usize> = (0..=file_lines.len().saturating_sub(search.len()))
        .filter(|&i| {
            file_lines[i..i + search.len()]
                .iter()
                .zip(search)
                .all(|(a, b)| a == b)
        })
        .collect();
    if let Some(pos) = disambiguate(&exact, hint) {
        return Some(pos);
    }
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
            let target = hint?.saturating_sub(1);
            Some(
                *positions
                    .iter()
                    .min_by_key(|&&p| p.abs_diff(target))
                    .unwrap(),
            )
        }
    }
}
