use std::io::ErrorKind;
use std::path::Path;

use loopal_error::LoopalError;

const MANAGED_BLOCK: &[u8] = b"# Loopal local-only settings\n/settings.local.json\n";

pub(super) fn ensure_local_settings_ignored(loopal_dir: &Path) -> Result<(), LoopalError> {
    let path = loopal_dir.join(".gitignore");
    let mut contents = match std::fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(LoopalError::Io(error)),
    };
    if contents.ends_with(MANAGED_BLOCK) {
        return Ok(());
    }
    if !contents.is_empty() && contents.last() != Some(&b'\n') {
        contents.push(b'\n');
    }
    contents.extend_from_slice(MANAGED_BLOCK);
    crate::atomic_settings_write::atomic_write(&path, &contents)
}
