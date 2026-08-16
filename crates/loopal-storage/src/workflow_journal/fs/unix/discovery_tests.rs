use std::ffi::CString;
use std::fs::File;
use std::os::fd::AsRawFd;

use super::{directory_stream, metadata_at};

#[test]
fn directory_stream_rejects_regular_and_closed_descriptors() {
    let temp = tempfile::tempdir().unwrap();
    let regular_path = temp.path().join("regular");
    std::fs::write(&regular_path, b"value").unwrap();
    let regular = File::open(&regular_path).unwrap();
    assert!(directory_stream(&regular).is_err());

    let closed = File::open(temp.path()).unwrap();
    assert_eq!(unsafe { libc::close(closed.as_raw_fd()) }, 0);
    assert!(directory_stream(&closed).is_err());
    std::mem::forget(closed);
}

#[test]
fn metadata_at_reports_invalid_parent_descriptor() {
    let name = CString::new("missing").unwrap();
    assert!(metadata_at(-1, &name).is_err());
}
