use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct FileSnapshot {
    path: PathBuf,
    label: String,
    content: String,
    mtime: Option<SystemTime>,
}

impl FileSnapshot {
    pub fn load(path: PathBuf, label: impl Into<String>) -> Self {
        let (content, mtime) = read_with_mtime(&path);
        Self {
            path,
            label: label.into(),
            content,
            mtime,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Check for changes and return a formatted reminder if the file content differs.
    /// Combines mtime check + content read + diff in a single call to avoid TOCTOU races.
    pub fn check_and_refresh(&mut self) -> Option<String> {
        let current_mtime = fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok());
        if current_mtime == self.mtime {
            return None;
        }
        // Stat first (above), then read content. If a concurrent write happens between
        // stat and read, we capture newer content with an older mtime. The next check
        // sees a newer mtime and re-reads — no writes are permanently missed.
        let new_content = fs::read_to_string(&self.path).unwrap_or_default();

        if new_content == self.content {
            self.mtime = current_mtime;
            return None;
        }
        let (added, removed) = line_diff(&self.content, &new_content);
        self.content = new_content;
        self.mtime = current_mtime;
        if added.is_empty() && removed.is_empty() {
            return None;
        }
        let added_refs: Vec<&str> = added.iter().map(String::as_str).collect();
        let removed_refs: Vec<&str> = removed.iter().map(String::as_str).collect();
        Some(format_file_change(&self.label, &added_refs, &removed_refs))
    }
}

fn read_with_mtime(path: &Path) -> (String, Option<SystemTime>) {
    let mtime = fs::metadata(path).ok().and_then(|m| m.modified().ok());
    let content = fs::read_to_string(path).unwrap_or_default();
    (content, mtime)
}

/// Ordered line diff that preserves duplicates.
/// Returns (lines only in new, lines only in old) maintaining original order.
pub fn line_diff(old: &str, new: &str) -> (Vec<String>, Vec<String>) {
    let old_lines: Vec<&str> = old.lines().filter(|l| !l.trim().is_empty()).collect();
    let new_lines: Vec<&str> = new.lines().filter(|l| !l.trim().is_empty()).collect();

    let mut old_counts = std::collections::HashMap::<&str, usize>::new();
    for l in &old_lines {
        *old_counts.entry(l).or_default() += 1;
    }
    let mut new_counts = std::collections::HashMap::<&str, usize>::new();
    for l in &new_lines {
        *new_counts.entry(l).or_default() += 1;
    }

    let mut added = Vec::new();
    let mut add_budget = std::collections::HashMap::<&str, usize>::new();
    for (&line, &new_n) in &new_counts {
        let old_n = old_counts.get(line).copied().unwrap_or(0);
        if new_n > old_n {
            add_budget.insert(line, new_n - old_n);
        }
    }
    for l in &new_lines {
        if let Some(n) = add_budget.get_mut(l)
            && *n > 0
        {
            added.push(l.to_string());
            *n -= 1;
        }
    }

    let mut removed = Vec::new();
    let mut rem_budget = std::collections::HashMap::<&str, usize>::new();
    for (&line, &old_n) in &old_counts {
        let new_n = new_counts.get(line).copied().unwrap_or(0);
        if old_n > new_n {
            rem_budget.insert(line, old_n - new_n);
        }
    }
    for l in &old_lines {
        if let Some(n) = rem_budget.get_mut(l)
            && *n > 0
        {
            removed.push(l.to_string());
            *n -= 1;
        }
    }
    (added, removed)
}

pub fn format_file_change(label: &str, added: &[&str], removed: &[&str]) -> String {
    let limit = 15;
    let mut parts = vec![format!("[Config Update] {label} changed:")];
    if !added.is_empty() {
        parts.push("  Added:".to_string());
        for line in added.iter().take(limit) {
            parts.push(format!("  + {line}"));
        }
        if added.len() > limit {
            parts.push(format!("  ... and {} more lines", added.len() - limit));
        }
    }
    if !removed.is_empty() {
        parts.push("  Removed:".to_string());
        for line in removed.iter().take(limit) {
            parts.push(format!("  - {line}"));
        }
        if removed.len() > limit {
            parts.push(format!("  ... and {} more lines", removed.len() - limit));
        }
    }
    parts.join("\n")
}
