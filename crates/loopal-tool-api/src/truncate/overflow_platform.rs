use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(unix)]
pub(super) fn set_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
pub(super) fn set_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_file_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
pub(super) fn set_file_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
pub(super) fn set_file_permissions(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
pub(super) fn set_file_permissions(_file: &File) -> io::Result<()> {
    Ok(())
}
