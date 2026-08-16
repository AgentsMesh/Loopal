use std::path::Path;

use loopal_error::ToolIoError;
use tokio::fs::File;

pub(crate) async fn prepare_directory(path: &Path) -> Result<(), ToolIoError> {
    if let Err(error) = tokio::fs::create_dir_all(path).await {
        return Err(io_error("create log directory", path, error));
    }
    ensure_private_directory(path).await
}

pub(crate) async fn create(path: &Path) -> Result<File, ToolIoError> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = match options.open(path).await {
        Ok(file) => file,
        Err(error) => return Err(io_error("create log file", path, error)),
    };
    ensure_private_file(path, &file).await?;
    Ok(file)
}

#[cfg(unix)]
async fn ensure_private_directory(path: &Path) -> Result<(), ToolIoError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) => return Err(io_error("inspect log directory", path, error)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(integrity_error(
            "log directory is not a regular directory",
            path,
        ));
    }
    if let Err(error) =
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await
    {
        return Err(io_error("secure log directory", path, error));
    }
    Ok(())
}

#[cfg(not(unix))]
async fn ensure_private_directory(_path: &Path) -> Result<(), ToolIoError> {
    Ok(())
}

#[cfg(unix)]
async fn ensure_private_file(path: &Path, file: &File) -> Result<(), ToolIoError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(error) => return Err(io_error("inspect log file", path, error)),
    };
    if !metadata.is_file() {
        return Err(integrity_error("log path is not a regular file", path));
    }
    if let Err(error) = file
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .await
    {
        return Err(io_error("secure log file", path, error));
    }
    Ok(())
}

#[cfg(not(unix))]
async fn ensure_private_file(_path: &Path, _file: &File) -> Result<(), ToolIoError> {
    Ok(())
}

fn io_error(action: &str, path: &Path, error: std::io::Error) -> ToolIoError {
    ToolIoError::ExecFailed(format!("{action} {}: {error}", path.display()))
}

fn integrity_error(reason: &str, path: &Path) -> ToolIoError {
    ToolIoError::ExecFailed(format!("{reason}: {}", path.display()))
}

#[cfg(test)]
#[path = "private_log_file_tests.rs"]
mod tests;
