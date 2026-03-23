mod parser;

pub use parser::{Skill, parse_skill};

use std::collections::HashMap;
use std::path::Path;

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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = format!("/{stem}");
        let skill = parse_skill(&name, &content);
        map.insert(name, skill);
    }
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
