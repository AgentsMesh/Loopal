use super::super::FsError;

pub(super) fn classify_open(error: std::io::Error) -> FsError {
    match error.raw_os_error() {
        Some(libc::ENOENT) => FsError::Missing,
        Some(libc::ELOOP | libc::ENOTDIR | libc::EEXIST) => {
            FsError::Integrity("workflow journal path changed or is not regular")
        }
        _ => FsError::Io(error),
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) fn errno() -> i32 {
    unsafe { *libc::__error() }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(super) fn clear_errno() {
    unsafe { *libc::__error() = 0 };
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub(super) fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub(super) fn clear_errno() {
    unsafe { *libc::__errno_location() = 0 };
}
