#[path = "../src/gate.rs"]
mod gate;
#[path = "../src/lcov.rs"]
mod lcov;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/path.rs"]
mod path;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use model::Manifest;

static NEXT: AtomicU64 = AtomicU64::new(0);
const A: &str = "crates/app/src/a.rs";
const B: &str = "crates/app/src/b.rs";

fn manifest() -> Manifest {
    Manifest {
        sources: BTreeSet::from([A.into(), B.into()]),
        critical: Vec::new(),
    }
}

fn fixture(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal-branch-supplement-{}-{}.lcov",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, text).unwrap();
    path
}

fn record(path: &str, line_hits: u64, function_hits: u64, branch_hits: u64) -> String {
    format!(
        "SF:{path}\nDA:1,{line_hits}\nFN:1,work\nFNDA:{function_hits},work\nBRDA:1,0,0,{branch_hits}\nend_of_record\n"
    )
}

fn branchless_record(path: &str, line_hits: u64, function_hits: u64) -> String {
    format!("SF:{path}\nDA:1,{line_hits}\nFN:1,work\nFNDA:{function_hits},work\nend_of_record\n")
}

fn baseline_record(path: &str) -> String {
    format!("SF:{path}\nend_of_record\n")
}

#[test]
fn branch_reports_union_without_inflating_base_metrics() {
    let sources = manifest();
    let base = fixture(&(record(A, 0, 0, 0) + &record(B, 1, 1, 0)));
    let branch_a = fixture(&record(A, 99, 99, 2));
    let branch_b = fixture(&record(B, 99, 99, 3));

    let mut coverage = lcov::parse(&base, Path::new("/workspace/Loopal"), &sources).unwrap();
    let branches = lcov::parse_many(
        &[branch_a.as_path(), branch_b.as_path()],
        Path::new("/workspace/Loopal"),
        &sources,
    )
    .unwrap();
    gate::apply_branch_supplements(&mut coverage, &branches, &sources).unwrap();

    assert_eq!(coverage.files[A].line_counter().hit, 0);
    assert_eq!(coverage.files[A].function_counter().hit, 0);
    assert_eq!(coverage.files[A].branch_counter().hit, 1);
    assert_eq!(coverage.files[B].branch_counter().hit, 1);

    for path in [base, branch_a, branch_b] {
        fs::remove_file(path).ok();
    }
}

#[test]
fn branch_union_missing_scoped_source_fails_closed() {
    let sources = manifest();
    let base = fixture(&(record(A, 1, 1, 0) + &record(B, 1, 1, 0)));
    let branch_a = fixture(&record(A, 1, 1, 1));
    let mut coverage = lcov::parse(&base, Path::new("/workspace/Loopal"), &sources).unwrap();
    let branches = lcov::parse_many(
        &[branch_a.as_path()],
        Path::new("/workspace/Loopal"),
        &sources,
    )
    .unwrap();

    let errors = gate::apply_branch_supplements(&mut coverage, &branches, &sources).unwrap_err();
    assert_eq!(
        errors,
        vec![format!("branch LCOV is missing included source: {B}")]
    );

    fs::remove_file(base).ok();
    fs::remove_file(branch_a).ok();
}

#[test]
fn parsing_zero_reports_is_rejected() {
    let sources = manifest();
    let paths: [&Path; 0] = [];
    assert!(
        lcov::parse_many(&paths, Path::new("/workspace/Loopal"), &sources)
            .unwrap_err()
            .contains("no LCOV inputs")
    );
}

#[test]
fn sf_only_branch_baseline_fails_closed() {
    let sources = manifest();
    let base = fixture(&(record(A, 1, 1, 0) + &record(B, 1, 1, 0)));
    let branch = fixture(&(record(A, 1, 1, 1) + &baseline_record(B)));
    let mut coverage = lcov::parse(&base, Path::new("/workspace/Loopal"), &sources).unwrap();
    let branches = lcov::parse_many(
        &[branch.as_path()],
        Path::new("/workspace/Loopal"),
        &sources,
    )
    .unwrap();

    let errors = gate::apply_branch_supplements(&mut coverage, &branches, &sources).unwrap_err();
    assert_eq!(
        errors,
        vec![format!(
            "branch LCOV has only an SF baseline for included source: {B}"
        )]
    );

    fs::remove_file(base).ok();
    fs::remove_file(branch).ok();
}

#[test]
fn instrumented_source_with_no_branches_is_accepted() {
    let sources = manifest();
    let base = fixture(&(record(A, 1, 1, 0) + &record(B, 1, 1, 0)));
    let branch = fixture(&(record(A, 1, 1, 1) + &branchless_record(B, 1, 1)));
    let mut coverage = lcov::parse(&base, Path::new("/workspace/Loopal"), &sources).unwrap();
    let branches = lcov::parse_many(
        &[branch.as_path()],
        Path::new("/workspace/Loopal"),
        &sources,
    )
    .unwrap();

    gate::apply_branch_supplements(&mut coverage, &branches, &sources).unwrap();
    assert_eq!(coverage.files[A].branch_counter().found, 1);
    assert_eq!(coverage.files[B].branch_counter().found, 0);

    fs::remove_file(base).ok();
    fs::remove_file(branch).ok();
}

#[test]
fn every_branch_input_must_contain_scoped_brda() {
    let sources = manifest();
    let branch = fixture(&record(A, 1, 1, 1));
    let ordinary = fixture(&branchless_record(B, 1, 1));

    let error = lcov::parse_many(
        &[branch.as_path(), ordinary.as_path()],
        Path::new("/workspace/Loopal"),
        &sources,
    )
    .unwrap_err();
    assert!(error.contains("branch LCOV contains no scoped BRDA records"));
    assert!(error.contains(&ordinary.display().to_string()));

    fs::remove_file(branch).ok();
    fs::remove_file(ordinary).ok();
}
