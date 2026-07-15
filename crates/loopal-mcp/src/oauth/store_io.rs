use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_CREDENTIAL_BYTES: u64 = 1024 * 1024;

pub(super) fn secure_read(path: &Path) -> std::io::Result<Option<String>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io_error(
            std::io::ErrorKind::PermissionDenied,
            "unsafe credential file",
        ));
    }
    if metadata.len() > MAX_CREDENTIAL_BYTES {
        return Err(io_error(
            std::io::ErrorKind::InvalidData,
            "credential file too large",
        ));
    }
    secure_parent(path)?;
    secure_file_permissions(path)?;
    let mut data = String::new();
    std::fs::File::open(path)?
        .take(MAX_CREDENTIAL_BYTES + 1)
        .read_to_string(&mut data)?;
    if data.len() > MAX_CREDENTIAL_BYTES as usize {
        return Err(io_error(
            std::io::ErrorKind::InvalidData,
            "credential file too large",
        ));
    }
    Ok(Some(data))
}

pub(super) fn secure_write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if data.len() > MAX_CREDENTIAL_BYTES as usize {
        return Err(io_error(
            std::io::ErrorKind::InvalidData,
            "credential data too large",
        ));
    }
    secure_parent(path)?;
    let parent = path.parent().ok_or_else(missing_parent)?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = parent.join(format!(".oauth-{}-{sequence}.tmp", std::process::id()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(data)?;
        file.sync_all()?;
        secure_file_permissions(&temp)?;
        replace_file(&temp, path)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn secure_parent(path: &Path) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(missing_parent)?;
    match std::fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(io_error(
                std::io::ErrorKind::PermissionDenied,
                "unsafe credential dir",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(parent)?;
        }
        Err(error) => return Err(error),
    }
    let metadata = std::fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io_error(
            std::io::ErrorKind::PermissionDenied,
            "unsafe credential dir",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn missing_parent() -> std::io::Error {
    io_error(
        std::io::ErrorKind::InvalidInput,
        "missing credential directory",
    )
}

fn io_error(kind: std::io::ErrorKind, message: &'static str) -> std::io::Error {
    std::io::Error::new(kind, message)
}

#[cfg(unix)]
fn secure_file_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn secure_file_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::rename(temp, target)
}

#[cfg(not(unix))]
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_file(target);
    std::fs::rename(temp, target)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    #[test]
    fn credential_write_is_atomic_private_and_bounded() {
        let root = std::env::temp_dir().join(format!(
            "loopal-oauth-test-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        let path = root.join("credentials.json");
        secure_write(&path, b"secret-marker").unwrap();
        secure_write(&path, b"replacement").unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(secure_read(&path).unwrap().as_deref(), Some("replacement"));
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let oversized = root.join("oversized.json");
        std::fs::write(&oversized, vec![b'x'; MAX_CREDENTIAL_BYTES as usize + 1]).unwrap();
        assert!(secure_read(&oversized).is_err());
        assert!(
            secure_write(
                &root.join("blocked.json"),
                &vec![b'x'; MAX_CREDENTIAL_BYTES as usize + 1]
            )
            .is_err()
        );

        let external = std::env::temp_dir().join(format!(
            "loopal-oauth-external-{}",
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&external).unwrap();
        let linked = root.join("linked-oauth");
        symlink(&external, &linked).unwrap();
        assert!(secure_write(&linked.join("credentials.json"), b"blocked").is_err());
        assert!(!external.join("credentials.json").exists());
        let _ = std::fs::remove_dir_all(external);
        let _ = std::fs::remove_dir_all(root);
    }
}
