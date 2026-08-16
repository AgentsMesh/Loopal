use std::ffi::{CStr, CString, OsString};
use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::ffi::OsStringExt;

use super::error::{classify_open, clear_errno, errno};
use super::{Entry, identity, validate_regular, workflows_directory};
use crate::workflow_journal::fs::FsError;

struct DirectoryStream(*mut libc::DIR);

impl Drop for DirectoryStream {
    fn drop(&mut self) {
        unsafe { libc::closedir(self.0) };
    }
}

pub(super) fn discover(base: &std::path::Path, session: &str) -> Result<Vec<Entry>, FsError> {
    let directory = workflows_directory(base, session, false)?;
    let stream = directory_stream(&directory)?;
    let mut entries = Vec::new();
    loop {
        clear_errno();
        let raw = unsafe { libc::readdir(stream.0) };
        if raw.is_null() {
            let error = errno();
            return if error == 0 {
                Ok(entries)
            } else {
                Err(FsError::Io(std::io::Error::from_raw_os_error(error)))
            };
        }
        let name = unsafe { CStr::from_ptr((*raw).d_name.as_ptr()) }.to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        let name = CString::new(name).map_err(|_| FsError::Integrity("invalid journal name"))?;
        let metadata = metadata_at(directory.as_raw_fd(), &name)?;
        validate_regular(&metadata)?;
        entries.push(Entry {
            name: OsString::from_vec(name.as_bytes().to_vec()),
            bytes: metadata.st_size.max(0) as u64,
            identity: identity(&metadata),
        });
    }
}

fn directory_stream(directory: &File) -> Result<DirectoryStream, FsError> {
    let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicate < 0 {
        return Err(FsError::Io(std::io::Error::last_os_error()));
    }
    let stream = unsafe { libc::fdopendir(duplicate) };
    if stream.is_null() {
        unsafe { libc::close(duplicate) };
        Err(FsError::Io(std::io::Error::last_os_error()))
    } else {
        Ok(DirectoryStream(stream))
    }
}

pub(super) fn metadata_at(parent: RawFd, name: &CStr) -> Result<libc::stat, FsError> {
    let mut value = std::mem::MaybeUninit::uninit();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            value.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        return Err(classify_open(std::io::Error::last_os_error()));
    }
    Ok(unsafe { value.assume_init() })
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
