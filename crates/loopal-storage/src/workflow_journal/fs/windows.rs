use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::mem::MaybeUninit;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
};

use super::{FsError, JournalLocation, OpenMode, parts};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity {
    volume: u32,
    index: u64,
}

pub(super) struct Opened {
    pub(super) file: File,
    pub(super) identity: FileIdentity,
}

pub(super) struct Entry {
    pub(super) name: OsString,
    pub(super) bytes: u64,
    pub(super) identity: FileIdentity,
}

pub(super) fn open(location: &JournalLocation, mode: OpenMode) -> Result<Opened, FsError> {
    let (base, session, name) = parts(location);
    let create = matches!(mode, OpenMode::AppendCreate);
    let (_guards, directory) = workflows_directory(base, session, create)?;
    let path = directory.join(name);
    let mut options = OpenOptions::new();
    match mode {
        OpenMode::Read => {
            options.read(true);
        }
        OpenMode::AppendCreate => {
            options.write(true).append(true).create_new(true);
        }
        OpenMode::AppendExisting => {
            options.read(true).write(true).append(true);
        }
        OpenMode::Repair => {
            options.write(true);
        }
    }
    secure_options(&mut options, false, matches!(mode, OpenMode::Read));
    let file = options.open(path).map_err(classify_open)?;
    validate_file(&file)
}

pub(super) fn discover(base: &Path, session: &str) -> Result<Vec<Entry>, FsError> {
    let (_guards, directory) = workflows_directory(base, session, false)?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&directory).map_err(classify_open)? {
        let entry = entry.map_err(FsError::Io)?;
        let name = entry.file_name();
        let mut options = OpenOptions::new();
        options.read(true);
        secure_options(&mut options, false, true);
        let file = options.open(directory.join(&name)).map_err(classify_open)?;
        let opened = validate_file(&file)?;
        let bytes = file.metadata().map_err(FsError::Io)?.len();
        entries.push(Entry {
            name,
            bytes,
            identity: opened.identity,
        });
    }
    Ok(entries)
}

fn workflows_directory(
    base: &Path,
    session: &str,
    create: bool,
) -> Result<(Vec<File>, PathBuf), FsError> {
    if create {
        std::fs::create_dir_all(base).map_err(FsError::Io)?;
    }
    let mut guards = Vec::new();
    let mut path = base.to_path_buf();
    guards.push(open_directory(&path)?);
    for component in ["sessions", session, "workflows"] {
        path.push(component);
        match open_directory(&path) {
            Ok(directory) => guards.push(directory),
            Err(FsError::Missing) if create => {
                match std::fs::create_dir(&path) {
                    Ok(()) => {}
                    Err(error)
                        if matches!(
                            error.raw_os_error().map(|value| value as u32),
                            Some(
                                windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS
                                    | windows_sys::Win32::Foundation::ERROR_FILE_EXISTS
                            )
                        ) => {}
                    Err(error) => return Err(FsError::Io(error)),
                }
                guards.push(open_directory(&path)?);
            }
            Err(error) => return Err(error),
        }
    }
    Ok((guards, path))
}

fn open_directory(path: &Path) -> Result<File, FsError> {
    let mut options = OpenOptions::new();
    options.read(true);
    secure_options(&mut options, true, true);
    let file = options.open(path).map_err(classify_open)?;
    let metadata = file.metadata().map_err(FsError::Io)?;
    use std::os::windows::fs::MetadataExt;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(FsError::Integrity(
            "workflow journal parent is not a private directory",
        ));
    }
    Ok(file)
}

fn secure_options(options: &mut OpenOptions, directory: bool, share_write: bool) {
    let mut flags = FILE_FLAG_OPEN_REPARSE_POINT;
    if directory {
        flags |= FILE_FLAG_BACKUP_SEMANTICS;
    }
    let share = FILE_SHARE_READ | if share_write { FILE_SHARE_WRITE } else { 0 };
    options.share_mode(share).custom_flags(flags);
}

fn validate_file(file: &File) -> Result<Opened, FsError> {
    let metadata = file.metadata().map_err(FsError::Io)?;
    use std::os::windows::fs::MetadataExt;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(FsError::Integrity(
            "workflow journal is not a private regular file",
        ));
    }
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    let handle = file.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    if unsafe { GetFileInformationByHandle(handle, information.as_mut_ptr()) } == 0 {
        return Err(FsError::Io(std::io::Error::last_os_error()));
    }
    let information = unsafe { information.assume_init() };
    if information.nNumberOfLinks != 1 {
        return Err(FsError::Integrity(
            "workflow journal is not a private regular file",
        ));
    }
    Ok(Opened {
        file: file.try_clone().map_err(FsError::Io)?,
        identity: FileIdentity {
            volume: information.dwVolumeSerialNumber,
            index: ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64,
        },
    })
}

fn classify_open(error: std::io::Error) -> FsError {
    use windows_sys::Win32::Foundation::{
        ERROR_ALREADY_EXISTS, ERROR_CANT_ACCESS_FILE, ERROR_DIRECTORY, ERROR_FILE_EXISTS,
        ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_REPARSE_TAG_INVALID,
    };
    match error.raw_os_error().map(|value| value as u32) {
        Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) => FsError::Missing,
        Some(
            ERROR_ALREADY_EXISTS
            | ERROR_CANT_ACCESS_FILE
            | ERROR_DIRECTORY
            | ERROR_FILE_EXISTS
            | ERROR_REPARSE_TAG_INVALID,
        ) => FsError::Integrity("workflow journal path changed or is not regular"),
        _ => FsError::Io(error),
    }
}
