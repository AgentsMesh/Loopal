use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

const HEADER: &str = "# Loopal memory runtime (auto-managed)";
const EVENTS_RULE: &str = ".loopal/memory-events/archive/";
const EVENTS_GZ_RULE: &str = ".loopal/memory-events/*.jsonl.gz";

pub fn ensure_gitignore(project_root: &Path) -> std::io::Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let existing_bytes = match std::fs::read(&gitignore_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let existing = String::from_utf8_lossy(&existing_bytes);
    let existing_lines: HashSet<&str> = existing.lines().map(str::trim).collect();
    let mut to_append: Vec<&str> = Vec::new();
    for rule in [EVENTS_RULE, EVENTS_GZ_RULE] {
        if !existing_lines.contains(rule) {
            to_append.push(rule);
        }
    }
    if to_append.is_empty() {
        return Ok(());
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)?;
    let needs_leading_newline = !existing_bytes.is_empty() && !existing_bytes.ends_with(b"\n");
    if needs_leading_newline {
        file.write_all(b"\n")?;
    }
    if !existing_lines.contains(HEADER) {
        writeln!(file, "{}", HEADER)?;
    }
    for rule in to_append {
        writeln!(file, "{}", rule)?;
    }
    Ok(())
}
