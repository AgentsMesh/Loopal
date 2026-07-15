use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const REPLACEFILE_WRITE_THROUGH: u32 = 1;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn ReplaceFileW(
        replaced: *const u16,
        replacement: *const u16,
        backup: *const u16,
        flags: u32,
        exclude: *const std::ffi::c_void,
        reserved: *const std::ffi::c_void,
    ) -> i32;
}

pub async fn replace_existing(replacement: &Path, replaced: &Path) -> io::Result<()> {
    let replacement = replacement.to_path_buf();
    let replaced = replaced.to_path_buf();
    tokio::task::spawn_blocking(move || replace(&replacement, &replaced))
        .await
        .map_err(io::Error::other)?
}

fn replace(replacement: &PathBuf, replaced: &PathBuf) -> io::Result<()> {
    let replaced = wide(replaced.as_os_str());
    let replacement = wide(replacement.as_os_str());
    let result = unsafe {
        ReplaceFileW(
            replaced.as_ptr(),
            replacement.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_WRITE_THROUGH,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}
