use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use loopal_output_guard::GuardedText;
use thiserror::Error;

use super::{humanize_size, needs_truncation, truncate_output};

#[path = "overflow_platform.rs"]
mod platform;
use platform::{set_directory_permissions, set_file_mode, set_file_permissions};

pub const MAX_OVERFLOW_FILE_BYTES: usize = 8 * 1024 * 1024;

static FILE_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct OverflowResult {
    pub display: String,
    pub overflowed: bool,
}

#[derive(Debug, Error)]
pub enum OverflowPersistenceError {
    #[error("guarded overflow is {actual_bytes} bytes; persisted limit is {max_bytes} bytes")]
    ByteLimitExceeded {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("overflow directory unavailable")]
    Directory(#[source] io::Error),
    #[error("overflow file unavailable")]
    Open(#[source] io::Error),
    #[error("overflow write failed")]
    Write(#[source] io::Error),
    #[error("overflow flush failed")]
    Flush(#[source] io::Error),
}

pub fn handle_overflow(
    output: &GuardedText,
    max_lines: usize,
    max_bytes: usize,
    label: &str,
) -> Result<OverflowResult, OverflowPersistenceError> {
    let text = output.as_str();
    if !needs_truncation(text, max_lines, max_bytes) {
        return Ok(OverflowResult {
            display: text.to_string(),
            overflowed: false,
        });
    }
    let path = persist(output, label)?;
    let preview = truncate_output(text, max_lines / 4, max_bytes / 4);
    let total = humanize_size(text.len());
    let path = path.to_string_lossy();
    Ok(OverflowResult {
        display: format!(
            "{preview}\n\n[Output too large for context ({total}). Full output saved to: {path}]\n\
             Use the Read tool to access the complete output if needed."
        ),
        overflowed: true,
    })
}

fn persist(output: &GuardedText, label: &str) -> Result<PathBuf, OverflowPersistenceError> {
    if output.as_str().len() > MAX_OVERFLOW_FILE_BYTES {
        return Err(OverflowPersistenceError::ByteLimitExceeded {
            actual_bytes: output.as_str().len(),
            max_bytes: MAX_OVERFLOW_FILE_BYTES,
        });
    }
    let root = std::env::temp_dir().join("loopal");
    prepare_directory(&root).map_err(OverflowPersistenceError::Directory)?;
    let directory = root.join("overflow");
    prepare_directory(&directory).map_err(OverflowPersistenceError::Directory)?;
    let (path, file) = create_file(&directory, &safe_label(label))?;
    write_file(&path, file, output.as_str().as_bytes())?;
    Ok(path)
}

fn write_file(
    path: &Path,
    mut writer: impl Write,
    content: &[u8],
) -> Result<(), OverflowPersistenceError> {
    let result = write_and_flush(&mut writer, content);
    if result.is_err() {
        drop(writer);
        let _ = std::fs::remove_file(path);
    }
    result
}

fn write_and_flush(
    writer: &mut impl Write,
    content: &[u8],
) -> Result<(), OverflowPersistenceError> {
    if let Err(error) = writer.write_all(content) {
        return Err(OverflowPersistenceError::Write(error));
    }
    if let Err(error) = writer.flush() {
        return Err(OverflowPersistenceError::Flush(error));
    }
    Ok(())
}

fn prepare_directory(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other("overflow path is not a directory"));
    }
    set_directory_permissions(path)
}

fn create_file(directory: &Path, label: &str) -> Result<(PathBuf, File), OverflowPersistenceError> {
    create_file_with(
        directory,
        label,
        || FILE_NONCE.fetch_add(1, Ordering::Relaxed),
        set_file_permissions,
    )
}

fn create_file_with(
    directory: &Path,
    label: &str,
    mut next_nonce: impl FnMut() -> u64,
    mut secure: impl FnMut(&File) -> io::Result<()>,
) -> Result<(PathBuf, File), OverflowPersistenceError> {
    for _ in 0..8 {
        let path = directory.join(format!(
            "{label}_{}_{}.txt",
            std::process::id(),
            next_nonce()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        set_file_mode(&mut options);
        match options.open(&path) {
            Ok(file) => {
                if let Err(error) = secure(&file) {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(OverflowPersistenceError::Open(error));
                }
                return Ok((path, file));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(OverflowPersistenceError::Open(error)),
        }
    }
    Err(OverflowPersistenceError::Open(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "overflow filename retries exhausted",
    )))
}

fn safe_label(label: &str) -> String {
    let label: String = label
        .chars()
        .take(64)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if label.is_empty() {
        "output".into()
    } else {
        label
    }
}

#[cfg(test)]
#[path = "overflow_tests.rs"]
mod tests;
