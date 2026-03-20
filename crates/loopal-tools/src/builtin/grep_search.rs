use std::{collections::BTreeSet, path::PathBuf};

use loopal_error::LoopalError;
use regex::Regex;
use walkdir::WalkDir;

/// Output format for grep results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Content,
    FilesWithMatches,
    Count,
}

impl OutputMode {
    pub fn from_str_opt(s: Option<&str>) -> Result<Self, LoopalError> {
        match s {
            None | Some("files_with_matches") => Ok(Self::FilesWithMatches),
            Some("content") => Ok(Self::Content),
            Some("count") => Ok(Self::Count),
            Some(other) => Err(LoopalError::Tool(
                loopal_error::ToolError::InvalidInput(format!(
                    "Invalid output_mode: {other}. Use content, files_with_matches, or count"
                )),
            )),
        }
    }
}

/// Parameters controlling search behavior and output formatting.
#[derive(Default)]
pub struct SearchOptions {
    pub context_before: usize,
    pub context_after: usize,
    pub multiline: bool,
    pub type_extensions: Option<Vec<String>>,
}

/// A single line in search output, either a match or a context line.
#[derive(Debug, Clone)]
pub struct MatchLine {
    pub line_num: usize,
    pub content: String,
    pub is_match: bool,
}

/// Collected grep results.
pub struct GrepResults {
    pub file_matches: Vec<(PathBuf, Vec<Vec<MatchLine>>)>,
    pub total_match_count: usize,
}

/// Map a type name (e.g. "rust", "js") to file extensions.
pub fn type_to_extensions(type_name: &str) -> Option<Vec<&'static str>> {
    Some(match type_name {
        "js" => vec!["js", "mjs", "cjs", "jsx"],
        "ts" => vec!["ts", "tsx", "mts", "cts"],
        "py" => vec!["py", "pyi"],
        "rust" => vec!["rs"],
        "go" => vec!["go"],
        "java" => vec!["java"],
        "c" => vec!["c", "h"],
        "cpp" => vec!["cpp", "cc", "cxx", "hpp", "hh", "h"],
        "rb" => vec!["rb"],
        "php" => vec!["php"],
        "swift" => vec!["swift"],
        "kt" => vec!["kt", "kts"],
        "md" => vec!["md", "markdown"],
        "json" => vec!["json"],
        "yaml" => vec!["yaml", "yml"],
        "toml" => vec!["toml"],
        "html" => vec!["html", "htm"],
        "css" => vec!["css"],
        _ => return None,
    })
}

/// Walk files, apply glob/type filter, search with regex, collect results.
pub fn search_files(
    search_path: &PathBuf,
    re: &Regex,
    include_glob: Option<&globset::GlobMatcher>,
    max_total_matches: usize,
    opts: &SearchOptions,
) -> GrepResults {
    let entries = collect_file_entries(search_path);
    let mut file_matches = Vec::new();
    let mut total = 0;

    for path in entries {
        if !matches_filters(&path, include_glob, opts.type_extensions.as_deref()) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        let lines: Vec<&str> = content.lines().collect();
        let match_indices = find_match_indices(&content, &lines, re, opts.multiline);
        if match_indices.is_empty() {
            continue;
        }
        total += match_indices.len();
        let groups =
            collect_context_groups(&lines, &match_indices, opts.context_before, opts.context_after);
        file_matches.push((path, groups));
        if total >= max_total_matches {
            break;
        }
    }

    GrepResults { file_matches, total_match_count: total }
}

fn collect_file_entries(search_path: &PathBuf) -> Vec<PathBuf> {
    if search_path.is_file() { return vec![search_path.clone()]; }
    WalkDir::new(search_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect()
}

fn matches_filters(
    path: &std::path::Path,
    include_glob: Option<&globset::GlobMatcher>,
    type_exts: Option<&[String]>,
) -> bool {
    if let Some(glob_matcher) = include_glob
        && let Some(name) = path.file_name()
        && !glob_matcher.is_match(name)
    {
        return false;
    }
    if let Some(exts) = type_exts {
        let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !exts.iter().any(|e| e == file_ext) {
            return false;
        }
    }
    true
}

fn find_match_indices(
    content: &str,
    lines: &[&str],
    re: &Regex,
    multiline: bool,
) -> BTreeSet<usize> {
    let mut indices = BTreeSet::new();
    if multiline {
        for mat in re.find_iter(content) {
            let start_line = content[..mat.start()].matches('\n').count();
            let end_line = content[..mat.end()].matches('\n').count();
            for idx in start_line..=end_line.min(lines.len().saturating_sub(1)) {
                indices.insert(idx);
            }
        }
    } else {
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                indices.insert(idx);
            }
        }
    }
    indices
}

fn collect_context_groups(
    lines: &[&str],
    match_indices: &BTreeSet<usize>,
    before: usize,
    after: usize,
) -> Vec<Vec<MatchLine>> {
    let last = lines.len().saturating_sub(1);
    let ranges: Vec<(usize, usize)> = match_indices
        .iter()
        .map(|&i| (i.saturating_sub(before), (i + after).min(last)))
        .collect();

    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in ranges {
        if let Some(prev) = merged.last_mut()
            && r.0 <= prev.1 + 1
        {
            prev.1 = prev.1.max(r.1);
            continue;
        }
        merged.push(r);
    }

    merged.iter().map(|&(start, end)| {
        (start..=end).map(|i| MatchLine {
            line_num: i + 1, content: lines[i].to_string(), is_match: match_indices.contains(&i),
        }).collect()
    }).collect()
}
