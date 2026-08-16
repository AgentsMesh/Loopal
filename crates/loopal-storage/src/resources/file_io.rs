use std::path::Path;

use loopal_error::StorageError;
use tokio::fs;
use tokio::io::AsyncReadExt;

pub(super) async fn existing_matches(path: &Path, expected: &[u8]) -> Result<bool, StorageError> {
    match open_regular_bounded(path, expected.len()).await {
        Ok((file, existing)) if existing == expected => {
            enforce_private_permissions(&file).await?;
            Ok(true)
        }
        Ok(_) | Err(StorageError::ResourceByteLimitExceeded { .. }) => Ok(false),
        Err(StorageError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn private_create_options() -> fs::OpenOptions {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
}

#[cfg(unix)]
pub(super) async fn enforce_private_permissions(file: &fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .await
}

#[cfg(not(unix))]
pub(super) async fn enforce_private_permissions(_file: &fs::File) -> std::io::Result<()> {
    Ok(())
}

pub(super) async fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    replace_file_inner(temp, target).await
}

pub(super) async fn read_regular_bounded(
    path: &Path,
    max_bytes: usize,
) -> Result<Vec<u8>, StorageError> {
    open_regular_bounded(path, max_bytes)
        .await
        .map(|(_, bytes)| bytes)
}

async fn open_regular_bounded(
    path: &Path,
    max_bytes: usize,
) -> Result<(fs::File, Vec<u8>), StorageError> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let mut file = options.open(path).await.map_err(open_error)?;
    let metadata = file.metadata().await?;
    if !is_regular_file(&metadata) {
        return Err(StorageError::ResourceIntegrity);
    }
    if metadata.len() > max_bytes as u64 {
        return Err(StorageError::ResourceByteLimitExceeded { max_bytes });
    }
    let limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(max_bytes.min(metadata.len() as usize));
    (&mut file).take(limit).read_to_end(&mut bytes).await?;
    if bytes.len() > max_bytes {
        return Err(StorageError::ResourceByteLimitExceeded { max_bytes });
    }
    Ok((file, bytes))
}

#[cfg(unix)]
fn set_no_follow(options: &mut fs::OpenOptions) {
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(windows)]
fn set_no_follow(options: &mut fs::OpenOptions) {
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(any(unix, windows)))]
fn set_no_follow(_options: &mut fs::OpenOptions) {}

#[cfg(windows)]
fn is_regular_file(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.is_file() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
}

#[cfg(not(windows))]
fn is_regular_file(metadata: &std::fs::Metadata) -> bool {
    metadata.is_file()
}

fn open_error(error: std::io::Error) -> StorageError {
    #[cfg(unix)]
    if error.raw_os_error() == Some(libc::ELOOP) {
        return StorageError::ResourceIntegrity;
    }
    StorageError::Io(error)
}

#[cfg(not(windows))]
async fn replace_file_inner(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target).await
}

#[cfg(windows)]
async fn replace_file_inner(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temp: Vec<u16> = temp
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let target: Vec<u16> = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    tokio::task::spawn_blocking(move || {
        let moved = unsafe {
            MoveFileExW(
                temp.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if moved == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    })
    .await
    .map_err(std::io::Error::other)?
}
