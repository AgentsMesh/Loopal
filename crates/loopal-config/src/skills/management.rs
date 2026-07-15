use std::path::Path;

use loopal_error::{ConfigError, LoopalError};

use super::management_fs::{
    compare_revision, lock_skills, read_required_file, revision, skill_path, validate_description,
    validate_revision,
};
use super::{SKILL_FILE_BYTE_LIMIT, Skill, parse_skill, read_skill_file};

#[derive(Debug, Clone)]
pub struct ManagedSkill {
    pub skill: Skill,
    pub revision: String,
    pub direct_regular_file: bool,
}

pub fn list_skill_documents(dir: &Path) -> Result<Vec<ManagedSkill>, LoopalError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        return Err(invalid("skills directory", "must be a directory"));
    }
    let direct_directory =
        std::fs::symlink_metadata(dir).is_ok_and(|metadata| metadata.file_type().is_dir());
    let mut documents = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(LoopalError::Io)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(content) = read_skill_file(&path) else {
            continue;
        };
        let direct = direct_directory && entry.file_type().is_ok_and(|kind| kind.is_file());
        documents.push(document(&format!("/{stem}"), content, direct));
    }
    documents.sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
    Ok(documents)
}

pub fn get_global_skill(user_dir: &Path, name: &str) -> Result<ManagedSkill, LoopalError> {
    let path = skill_path(user_dir, name, false)?;
    let content = read_required_file(&path)?;
    Ok(document(name, content, true))
}

pub fn upsert_global_skill(
    user_dir: &Path,
    name: &str,
    description: &str,
    body: &str,
    expected_revision: Option<&str>,
) -> Result<ManagedSkill, LoopalError> {
    validate_description(description)?;
    if body.is_empty() || body.as_bytes().contains(&0) {
        return Err(invalid(
            "body",
            "must be non-empty and contain no NUL bytes",
        ));
    }
    let content = format!("---\ndescription: {description}\n---\n{body}");
    if content.len() as u64 > SKILL_FILE_BYTE_LIMIT {
        return Err(invalid("body", "serialized skill exceeds 100 KiB"));
    }
    let path = skill_path(user_dir, name, true)?;
    let _lock = lock_skills(path.parent().unwrap())?;
    compare_revision(&path, expected_revision)?;
    crate::atomic_settings_write::atomic_write(&path, content.as_bytes())?;
    Ok(document(name, content, true))
}

pub fn delete_global_skill(
    user_dir: &Path,
    name: &str,
    expected_revision: &str,
) -> Result<(), LoopalError> {
    validate_revision(expected_revision)?;
    let path = skill_path(user_dir, name, false)?;
    let _lock = lock_skills(path.parent().unwrap())?;
    compare_revision(&path, Some(expected_revision))?;
    std::fs::remove_file(path).map_err(LoopalError::Io)
}

fn document(name: &str, content: String, direct_regular_file: bool) -> ManagedSkill {
    ManagedSkill {
        skill: parse_skill(name, &content),
        revision: revision(content.as_bytes()),
        direct_regular_file,
    }
}

fn invalid(field: &str, reason: &str) -> LoopalError {
    ConfigError::InvalidValue {
        field: field.into(),
        reason: reason.into(),
    }
    .into()
}
