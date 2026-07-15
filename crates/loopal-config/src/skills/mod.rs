mod management;
mod management_fs;
mod parser;

pub use management::{
    ManagedSkill, delete_global_skill, get_global_skill, list_skill_documents, upsert_global_skill,
};
pub use parser::{Skill, parse_skill};

use std::collections::HashMap;
use std::path::Path;

pub const SKILL_FILE_BYTE_LIMIT: u64 = 100 * 1024;

/// Scan a single directory for `.md` skill files and return them sorted.
///
/// Used by the isomorphic layer loader to collect skills from any directory.
pub fn scan_skills_dir(dir: &Path) -> Vec<Skill> {
    let mut map = HashMap::new();
    load_skills_from_dir(dir, &mut map);
    let mut skills: Vec<Skill> = map.into_values().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Scan a directory for `.md` files and parse each as a skill.
fn load_skills_from_dir(dir: &Path, map: &mut HashMap<String, Skill>) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(content) = read_skill_file(&path) else {
            continue;
        };
        let name = format!("/{stem}");
        let skill = parse_skill(&name, &content);
        map.insert(name, skill);
    }
}

pub(crate) fn read_skill_file(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > SKILL_FILE_BYTE_LIMIT {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Format a human-readable skills summary for the system prompt.
pub fn format_skills_summary(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut s = String::from("# Available Skills\nUser can invoke these via /name:\n");
    for skill in skills {
        s.push_str(&format!("- {}: {}\n", skill.name, skill.description));
    }
    s
}

pub fn expand_skill(body: &str, args: &str) -> String {
    let args = args.trim();
    if body.contains("$ARGUMENTS") {
        body.replace("$ARGUMENTS", args)
    } else if args.is_empty() {
        body.to_string()
    } else {
        format!("{body}\n{args}")
    }
}
