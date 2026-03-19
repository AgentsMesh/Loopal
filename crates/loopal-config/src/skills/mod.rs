mod parser;

pub use parser::{Skill, parse_skill};

use std::collections::HashMap;
use std::path::Path;

use crate::locations::{global_skills_dir, project_skills_dir};

/// Load all skills from global and project directories.
///
/// Project skills override global skills with the same name.
pub fn load_skills(cwd: &Path) -> Vec<Skill> {
    let mut map = HashMap::new();

    // Global skills (lower priority)
    if let Ok(dir) = global_skills_dir() {
        load_skills_from_dir(&dir, &mut map);
    }

    // Project skills (higher priority, overrides global)
    let project_dir = project_skills_dir(cwd);
    load_skills_from_dir(&project_dir, &mut map);

    let mut skills: Vec<Skill> = map.into_values().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Scan a directory for `.md` files and parse each as a skill.
fn load_skills_from_dir(dir: &Path, map: &mut HashMap<String, Skill>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };

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
