use std::fs;

use loopal_output_guard::{GuardedText, OutputGuard};
use loopal_tool_api::{
    MAX_OVERFLOW_FILE_BYTES, OverflowPersistenceError, extract_overflow_path, handle_overflow,
};

fn guarded(text: &str) -> GuardedText {
    OutputGuard::new(&[])
        .unwrap()
        .guard_text(text, usize::MAX)
        .unwrap()
        .into_inner()
}

#[test]
fn small_output_stays_inline() {
    let result = handle_overflow(&guarded("small"), 10, 100, "Read").unwrap();

    assert!(!result.overflowed);
    assert_eq!(result.display, "small");
}

#[test]
fn overflow_persists_guarded_text_with_safe_name() {
    let output = guarded(&"safe-value\n".repeat(100));
    let result = handle_overflow(&output, 2, 20, "../unsafe label").unwrap();
    let (_, path) = extract_overflow_path(&result.display);
    let path = path.expect("overflow path");

    assert_eq!(fs::read_to_string(&path).unwrap(), output.as_str());
    let filename = std::path::Path::new(&path)
        .file_name()
        .unwrap()
        .to_string_lossy();
    assert!(filename.starts_with("___unsafe_label_"));
    fs::remove_file(path).unwrap();
}

#[test]
fn overflow_paths_are_collision_safe() {
    let output = guarded(&"value\n".repeat(20));
    let first = handle_overflow(&output, 1, 8, "same").unwrap();
    let second = handle_overflow(&output, 1, 8, "same").unwrap();
    let (_, first_path) = extract_overflow_path(&first.display);
    let (_, second_path) = extract_overflow_path(&second.display);

    assert_ne!(first_path, second_path);
    fs::remove_file(first_path.unwrap()).unwrap();
    fs::remove_file(second_path.unwrap()).unwrap();
}

#[test]
fn persisted_byte_limit_fails_closed() {
    let output = guarded(&"x".repeat(MAX_OVERFLOW_FILE_BYTES + 1));
    let error = handle_overflow(&output, 1, 1, "large").unwrap_err();

    assert!(matches!(
        error,
        OverflowPersistenceError::ByteLimitExceeded { .. }
    ));
}

#[cfg(unix)]
#[test]
fn overflow_file_and_directory_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let output = guarded(&"private\n".repeat(20));
    let result = handle_overflow(&output, 1, 8, "private").unwrap();
    let (_, path) = extract_overflow_path(&result.display);
    let path = std::path::PathBuf::from(path.unwrap());

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::remove_file(path).unwrap();
}
