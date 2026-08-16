use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

fn temp_dir() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let path = std::env::temp_dir().join(format!(
        "loopal-runfiles-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn resolves_escaped_manifest_entry() {
    let root = temp_dir();
    let physical = root.join("physical dir").join("loopal binary");
    std::fs::create_dir_all(physical.parent().unwrap()).unwrap();
    std::fs::write(&physical, "fixture").unwrap();
    let manifest = root.join("MANIFEST");
    let logical = "_main/loopal binary";
    std::fs::write(
        &manifest,
        format!(
            " {} {}\n_main/other ignored\n",
            escape_manifest_field(logical),
            escape_manifest_field(&physical.to_string_lossy()),
        ),
    )
    .unwrap();
    let locations = RunfilesLocations {
        manifest: Some(manifest),
        ..Default::default()
    };

    let resolved =
        resolve_configured_file("LOOPAL_BINARY", OsStr::new(logical), &locations).unwrap();

    assert_eq!(resolved, std::fs::canonicalize(&physical).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

fn escape_manifest_field(value: &str) -> String {
    value
        .replace('\\', "\\b")
        .replace(' ', "\\s")
        .replace('\n', "\\n")
}

#[test]
fn decodes_all_bazel_manifest_escape_sequences() {
    let (logical, physical) = parse_manifest_entry(r" _main/a\sb\nc\bd /tmp/x\sy\nz\bw").unwrap();

    assert_eq!(logical, "_main/a b\nc\\d");
    assert_eq!(physical, "/tmp/x y\nz\\w");
}

#[test]
fn runfiles_dir_precedes_test_srcdir() {
    let root = temp_dir();
    let logical = PathBuf::from("_main/loopal");
    let runfiles_file = root.join("runfiles").join(&logical);
    let test_srcdir_file = root.join("test-srcdir").join(&logical);
    for file in [&runfiles_file, &test_srcdir_file] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "fixture").unwrap();
    }
    let locations = RunfilesLocations {
        runfiles_dir: Some(root.join("runfiles")),
        test_srcdir: Some(root.join("test-srcdir")),
        manifest: None,
    };

    let resolved =
        resolve_configured_file("LOOPAL_BINARY", logical.as_os_str(), &locations).unwrap();

    assert_eq!(resolved, std::fs::canonicalize(&runfiles_file).unwrap());
    std::fs::remove_file(runfiles_file).unwrap();
    let resolved =
        resolve_configured_file("LOOPAL_BINARY", logical.as_os_str(), &locations).unwrap();
    assert_eq!(resolved, std::fs::canonicalize(test_srcdir_file).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_absolute_path_does_not_fall_back_to_manifest() {
    let root = temp_dir();
    let physical = root.join("loopal");
    std::fs::write(&physical, "fixture").unwrap();
    let missing = root.join("missing-loopal");
    let manifest = root.join("MANIFEST");
    std::fs::write(
        &manifest,
        format!("{} {}\n", missing.display(), physical.display()),
    )
    .unwrap();
    let locations = RunfilesLocations {
        manifest: Some(manifest),
        ..Default::default()
    };

    let error = resolve_configured_file("LOOPAL_BINARY", missing.as_os_str(), &locations)
        .unwrap_err()
        .to_string();

    assert!(error.contains("LOOPAL_BINARY"), "{error}");
    assert!(error.contains(&missing.display().to_string()), "{error}");
    std::fs::remove_dir_all(root).unwrap();
}
