use std::path::{Path, PathBuf};

use loopal_error::{ConfigError, LoopalError};
use sha2::{Digest, Sha256};

use super::{SKILL_FILE_BYTE_LIMIT, read_skill_file};
use crate::settings_file_lock::SettingsFileLock;

pub(super) fn skill_path(
    user_dir: &Path,
    name: &str,
    create: bool,
) -> Result<PathBuf, LoopalError> {
    let slug = validate_name(name)?;
    if create {
        ensure_real_directory(user_dir, "Loopal user directory")?;
        ensure_real_directory(&user_dir.join("skills"), "skills directory")?;
    } else {
        require_real_directory(user_dir, "Loopal user directory")?;
        require_real_directory(&user_dir.join("skills"), "skills directory")?;
    }
    Ok(user_dir.join("skills").join(format!("{slug}.md")))
}

pub(super) fn validate_description(value: &str) -> Result<(), LoopalError> {
    if value.trim().is_empty()
        || value.encode_utf16().count() > 512
        || value.contains('\r')
        || value.contains('\n')
        || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "description",
            "must be one non-empty control-free line up to 512 UTF-16 code units",
        ));
    }
    Ok(())
}

pub(super) fn compare_revision(path: &Path, expected: Option<&str>) -> Result<(), LoopalError> {
    reject_non_regular_target(path)?;
    let current = match std::fs::read(path) {
        Ok(bytes) => Some(revision(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(LoopalError::Io(error)),
    };
    if let Some(value) = expected {
        validate_revision(value)?;
    }
    if current.as_deref() != expected {
        return Err(invalid("expectedRevision", "skill revision conflict"));
    }
    Ok(())
}

pub(super) fn read_required_file(path: &Path) -> Result<String, LoopalError> {
    reject_non_regular_target(path)?;
    read_skill_file(path).ok_or_else(|| invalid("name", "global skill not found or invalid"))
}

pub(super) fn lock_skills(dir: &Path) -> Result<SettingsFileLock, LoopalError> {
    let path = dir.join(".skills.lock");
    reject_non_regular_target(&path)?;
    SettingsFileLock::acquire(path)
}

pub(super) fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn validate_revision(value: &str) -> Result<(), LoopalError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid("expectedRevision", "must be a SHA-256 revision"))
    }
}

fn validate_name(name: &str) -> Result<&str, LoopalError> {
    let Some(slug) = name.strip_prefix('/') else {
        return Err(invalid("name", "must begin with '/'"));
    };
    let mut chars = slug.chars();
    let valid = slug.len() <= 64
        && chars
            .next()
            .is_some_and(|value| value.is_ascii_alphanumeric())
        && chars.all(|value| value.is_ascii_alphanumeric() || matches!(value, '_' | '-'));
    if valid {
        Ok(slug)
    } else {
        Err(invalid("name", "must be a canonical ASCII skill name"))
    }
}

fn reject_non_regular_target(path: &Path) -> Result<(), LoopalError> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.file_type().is_file() => {
            Err(invalid("name", "skill is not a regular file"))
        }
        Ok(meta) if meta.len() > SKILL_FILE_BYTE_LIMIT => {
            Err(invalid("name", "skill exceeds 100 KiB"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(LoopalError::Io(error)),
    }
}

fn ensure_real_directory(path: &Path, field: &str) -> Result<(), LoopalError> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(LoopalError::Io)?;
    }
    require_real_directory(path, field)
}

fn require_real_directory(path: &Path, field: &str) -> Result<(), LoopalError> {
    let metadata = std::fs::symlink_metadata(path).map_err(LoopalError::Io)?;
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(invalid(field, "must be a real directory, not a symlink"))
    }
}

fn invalid(field: &str, reason: &str) -> LoopalError {
    ConfigError::InvalidValue {
        field: field.into(),
        reason: reason.into(),
    }
    .into()
}
