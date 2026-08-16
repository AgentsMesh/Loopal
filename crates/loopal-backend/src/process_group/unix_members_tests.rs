#![cfg(target_os = "macos")]

use super::{next_capacity, process_is_live};

#[test]
fn member_buffer_growth_is_bounded() {
    assert_eq!(next_capacity(64).unwrap(), 128);
    assert!(next_capacity(65_536).is_err());
    assert!(next_capacity(usize::MAX).is_err());
}

#[test]
fn process_probe_distinguishes_current_and_missing_pid() {
    assert!(process_is_live(std::process::id() as i32).unwrap());
    assert!(!process_is_live(i32::MAX).unwrap());
}
