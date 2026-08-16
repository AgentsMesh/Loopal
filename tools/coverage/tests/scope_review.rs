#[path = "../src/scope_review.rs"]
mod scope_review;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = "crates/loopal-runtime/src/tool_action.rs";
const OTHER: &str = "crates/loopal-runtime/src/tool_prepare.rs";

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal-scope-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(path.join("crates/loopal-runtime/src")).unwrap();
    fs::write(path.join(SOURCE), "fn action() {}\n").unwrap();
    fs::write(path.join(OTHER), "fn prepare() {}\n").unwrap();
    path
}

fn exclusions(root: &std::path::Path, rows: &str) -> PathBuf {
    let path = root.join("scope_exclusions.txt");
    fs::write(&path, rows).unwrap();
    path
}

#[test]
fn included_changed_source_passes() {
    let root = fixture();
    let included = BTreeSet::from([SOURCE.into()]);
    let result = scope_review::review([SOURCE.into()], &included, &exclusions(&root, ""), &root);
    assert!(result.is_ok());
    fs::remove_dir_all(root).ok();
}

#[test]
fn deleted_unlisted_source_is_a_reviewed_removal() {
    let root = fixture();
    fs::remove_file(root.join(SOURCE)).unwrap();

    let result = scope_review::review(
        [SOURCE.into()],
        &BTreeSet::new(),
        &exclusions(&root, ""),
        &root,
    );

    assert!(result.is_ok());
    fs::remove_dir_all(root).ok();
}

#[test]
fn deleted_included_source_fails_closed() {
    let root = fixture();
    fs::remove_file(root.join(SOURCE)).unwrap();

    let errors = scope_review::review(
        [SOURCE.into()],
        &BTreeSet::from([SOURCE.into()]),
        &exclusions(&root, ""),
        &root,
    )
    .unwrap_err();

    assert_eq!(
        errors,
        vec![format!("included source was deleted: {SOURCE}")]
    );
    fs::remove_dir_all(root).ok();
}

#[test]
fn unreviewed_changed_source_fails() {
    let root = fixture();
    let result = scope_review::review(
        [SOURCE.into()],
        &BTreeSet::new(),
        &exclusions(&root, ""),
        &root,
    );
    assert!(result.unwrap_err()[0].contains("unreviewed"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn content_hash_makes_exclusion_stale_after_edit() {
    let root = fixture();
    let hash = scope_review::hash_file(&root.join(SOURCE)).unwrap();
    let rows = format!("{SOURCE}|{hash}|compatibility glue outside the effect boundary\n");
    let excluded = exclusions(&root, &rows);
    assert!(scope_review::review([SOURCE.into()], &BTreeSet::new(), &excluded, &root).is_ok());
    fs::write(root.join(SOURCE), "fn changed() {}\n").unwrap();
    let errors =
        scope_review::review([SOURCE.into()], &BTreeSet::new(), &excluded, &root).unwrap_err();
    assert!(errors[0].contains("stale"));
    fs::remove_dir_all(root).ok();
}

#[test]
fn tests_and_unrelated_crates_are_not_candidates() {
    assert!(!scope_review::review_candidate(
        "crates/loopal-runtime/tests/suite.rs"
    ));
    assert!(!scope_review::review_candidate(
        "crates/loopal-runtime/src/tests_security.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-tui/src/view.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-tui/src/views/workflows_panel.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-turn/src/turn.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-agent/src/workflow_control.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-backend/src/log_writer.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-output-guard/src/stream.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-secret-client/src/hub_client/authority.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-provider-api/src/wire/origin.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/loopal-workflow-schema/src/schema.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/tools/filesystem/fetch/src/lib.rs"
    ));
    assert!(scope_review::review_candidate(
        "crates/tools/process/bash/src/process_exec.rs"
    ));
    assert!(scope_review::review_candidate(SOURCE));
}

#[test]
fn malformed_exclusion_fails_closed() {
    let root = fixture();
    let excluded = exclusions(&root, &format!("{SOURCE}|bad|no\n"));
    let errors =
        scope_review::review([SOURCE.into()], &BTreeSet::new(), &excluded, &root).unwrap_err();
    assert!(errors[0].contains("malformed"));
    fs::remove_dir_all(root).ok();
}
