use std::ffi::{CString, OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[cfg(test)]
mod branch_tests;
mod discovery;
mod error;
#[cfg(test)]
mod tests;

use super::{FsError, JournalLocation, OpenMode, parts};
use error::classify_open;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FileIdentity {
    device: libc::dev_t,
    inode: libc::ino_t,
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
    let directory = workflows_directory(base, session, create)?;
    open_journal_at(&directory, name, mode)
}

fn open_journal_at(directory: &File, name: &OsStr, mode: OpenMode) -> Result<Opened, FsError> {
    let flags = match mode {
        OpenMode::Read => libc::O_RDONLY,
        OpenMode::AppendCreate => libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_APPEND,
        OpenMode::AppendExisting => libc::O_RDWR | libc::O_APPEND,
        OpenMode::Repair => libc::O_RDWR,
    } | libc::O_CLOEXEC
        | libc::O_NOFOLLOW;
    let file = open_at(directory.as_raw_fd(), name, flags, 0o600)?;
    lock(&file, mode)?;
    let metadata = metadata(&file)?;
    validate_regular(&metadata)?;
    Ok(Opened {
        file,
        identity: identity(&metadata),
    })
}

pub(super) fn discover(base: &Path, session: &str) -> Result<Vec<Entry>, FsError> {
    discovery::discover(base, session)
}

fn workflows_directory(base: &Path, session: &str, create: bool) -> Result<File, FsError> {
    if create {
        std::fs::create_dir_all(base).map_err(FsError::Io)?;
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut directory = options.open(base).map_err(classify_open)?;
    for component in [
        OsStr::new("sessions"),
        OsStr::new(session),
        OsStr::new("workflows"),
    ] {
        directory = open_directory_at(&directory, component, create)?;
    }
    Ok(directory)
}

fn open_directory_at(parent: &File, name: &OsStr, create: bool) -> Result<File, FsError> {
    let name = cstring(name)?;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    match open_at(
        parent.as_raw_fd(),
        OsStr::from_bytes(name.as_bytes()),
        flags,
        0,
    ) {
        Ok(directory) => Ok(directory),
        Err(FsError::Missing) if create => {
            let created = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
            if created < 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::EEXIST) {
                    return Err(FsError::Io(error));
                }
            }
            open_at(
                parent.as_raw_fd(),
                OsStr::from_bytes(name.as_bytes()),
                flags,
                0,
            )
        }
        Err(error) => Err(error),
    }
}

fn open_at(parent: RawFd, name: &OsStr, flags: i32, mode: libc::mode_t) -> Result<File, FsError> {
    let name = cstring(name)?;
    let fd = unsafe { libc::openat(parent, name.as_ptr(), flags, mode as libc::c_uint) };
    if fd < 0 {
        return Err(classify_open(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn lock(file: &File, mode: OpenMode) -> Result<(), FsError> {
    let operation = if matches!(mode, OpenMode::Read) {
        libc::LOCK_SH
    } else {
        libc::LOCK_EX
    };
    if unsafe { libc::flock(file.as_raw_fd(), operation) } < 0 {
        Err(FsError::Io(std::io::Error::last_os_error()))
    } else {
        Ok(())
    }
}

fn metadata(file: &File) -> Result<libc::stat, FsError> {
    let mut value = MaybeUninit::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), value.as_mut_ptr()) } < 0 {
        return Err(FsError::Io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { value.assume_init() })
}

fn validate_regular(metadata: &libc::stat) -> Result<(), FsError> {
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG || metadata.st_nlink != 1 {
        Err(FsError::Integrity(
            "workflow journal is not a private regular file",
        ))
    } else {
        Ok(())
    }
}

fn identity(metadata: &libc::stat) -> FileIdentity {
    FileIdentity {
        device: metadata.st_dev,
        inode: metadata.st_ino,
    }
}

fn cstring(value: &OsStr) -> Result<CString, FsError> {
    CString::new(value.as_bytes()).map_err(|_| FsError::Integrity("invalid journal path component"))
}
