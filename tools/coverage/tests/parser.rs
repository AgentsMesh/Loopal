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

use model::{CriticalFunction, Manifest};

static NEXT: AtomicU64 = AtomicU64::new(0);
const A: &str = "crates/app/src/a.rs";

fn manifest() -> Manifest {
    Manifest {
        sources: BTreeSet::from([A.into()]),
        critical: vec![CriticalFunction {
            path: A.into(),
            name: "critical".into(),
        }],
    }
}

fn parse(text: &str) -> Result<model::Coverage, String> {
    let path = fixture(text);
    let result = lcov::parse(&path, Path::new("/workspace/Loopal"), &manifest());
    fs::remove_file(path).ok();
    result
}

fn fixture(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal-coverage-parser-{}-{}.lcov",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, text).unwrap();
    path
}

fn record(path: &str, line: u32, hits: u64, function_hits: u64) -> String {
    format!(
        "SF:{path}\nDA:{line},{hits}\nFN:{line},critical\nFNDA:{function_hits},critical\nBRDA:{line},0,0,{hits}\nend_of_record\n"
    )
}

#[test]
fn duplicate_sf_records_merge_hits_by_identity() {
    let text = record(A, 1, 0, 0) + &record(A, 1, 3, 2);
    let coverage = parse(&text).unwrap();
    let file = &coverage.files[A];
    assert_eq!(file.lines[&1], 3);
    assert_eq!(file.functions["critical"], 2);
    assert_eq!(file.branches[&(1, 0, 0)], Some(3));
}

#[test]
fn function_counter_merges_rust_monomorphs_on_one_source_line() {
    let text = format!(
        "SF:{A}\nDA:7,1\nFN:7,_RNvNtC4hash_a8finalize\nFNDA:0,_RNvNtC4hash_a8finalize\nFN:7,_RNvNtC4hash_b8finalize\nFNDA:3,_RNvNtC4hash_b8finalize\nBRDA:7,0,0,1\nend_of_record\n"
    );
    let coverage = parse(&text).unwrap();
    assert_eq!(coverage.files[A].function_counter().found, 1);
    assert_eq!(coverage.files[A].function_counter().hit, 1);
}

#[test]
fn function_counter_excludes_compiler_generated_bodies() {
    let text = format!(
        "SF:{A}\nDA:7,1\nFN:7,_RNvNtC4demo4work\nFNDA:1,_RNvNtC4demo4work\nFN:8,_RNCNvNtC4demo4work0B1_\nFNDA:0,_RNCNvNtC4demo4work0B1_\nBRDA:7,0,0,1\nend_of_record\n"
    );
    let coverage = parse(&text).unwrap();
    let counter = coverage.files[A].function_counter();
    assert_eq!(counter.found, 1);
    assert_eq!(counter.hit, 1);
}

#[test]
fn generated_body_hits_do_not_mask_uncovered_source_function() {
    let text = format!(
        "SF:{A}\nDA:7,1\nFN:7,_RNvNtC4demo4work\nFNDA:0,_RNvNtC4demo4work\nFN:7,_RNCNvNtC4demo4work0B1_\nFNDA:3,_RNCNvNtC4demo4work0B1_\nBRDA:7,0,0,1\nend_of_record\n"
    );
    let coverage = parse(&text).unwrap();
    let counter = coverage.files[A].function_counter();
    assert_eq!(counter.found, 1);
    assert_eq!(counter.hit, 0);
}

#[test]
fn parses_indexed_lcov_function_records() {
    let text = format!(
        "SF:{A}\nDA:7,1\nFNL:0,7\nFNA:0,3,_RNvNtC4demo8critical\nBRDA:7,0,0,1\nend_of_record\n"
    );
    let coverage = parse(&text).unwrap();
    assert_eq!(coverage.files[A].functions["_RNvNtC4demo8critical"], 3);
    assert_eq!(coverage.files[A].function_lines["_RNvNtC4demo8critical"], 7);
}

#[test]
fn rejects_malformed_indexed_function_records() {
    for text in [
        format!("SF:{A}\nFNL:0,7\nend_of_record\n"),
        format!("SF:{A}\nFNA:0,1,name\nend_of_record\n"),
        format!("SF:{A}\nFNL:0,7\nFNL:0,8\nFNA:0,1,name\nend_of_record\n"),
        format!("SF:{A}\nFNL:0,7\nFNA:0,nope,name\nend_of_record\n"),
        format!("SF:{A}\nFN:7,name\nFNDA:1,name\nFNL:0,7\nFNA:0,1,name2\nend_of_record\n"),
    ] {
        assert!(parse(&text).is_err(), "accepted: {text}");
    }
}

#[test]
fn normalizes_workspace_absolute_and_execroot_paths() {
    let text = record("/workspace/Loopal/crates/app/src/a.rs", 1, 1, 1)
        + &record(
            "/private/tmp/bazel/execroot/_main/crates/app/src/./a.rs",
            2,
            1,
            1,
        );
    let coverage = parse(&text).unwrap();
    assert_eq!(coverage.files.len(), 1);
    assert_eq!(coverage.files[A].lines.len(), 2);
}

#[test]
fn filters_tests_external_generated_and_unlisted_sources() {
    let ignored = [
        "crates/app/tests/a.rs",
        "/tmp/execroot/_main/external/crate/src/lib.rs",
        "bazel-out/genfiles/generated.rs",
        "crates/app/src/unlisted.rs",
    ];
    let mut text = record(A, 1, 1, 1);
    for path in ignored {
        text += &record(path, 1, 0, 0);
    }
    let coverage = parse(&text).unwrap();
    assert_eq!(coverage.files.keys().collect::<Vec<_>>(), vec![A]);
}

#[test]
fn rejects_missing_empty_and_no_scoped_data() {
    let missing = fixture("");
    fs::remove_file(&missing).unwrap();
    assert!(lcov::parse(&missing, Path::new("/workspace/Loopal"), &manifest()).is_err());
    assert!(parse("").unwrap_err().contains("no scoped"));
    let ignored = record("external/x/src/lib.rs", 1, 1, 1);
    assert!(parse(&ignored).unwrap_err().contains("no scoped"));
}

#[test]
fn rejects_malformed_relevant_records() {
    let cases = [
        "DA:1,1\n",
        "SF:crates/app/src/a.rs\nDA:not-a-line,1\nend_of_record\n",
        "SF:crates/app/src/a.rs\nFN:1,critical\nFNDA:nope,critical\nend_of_record\n",
        "SF:crates/app/src/a.rs\nBRDA:1,0\nend_of_record\n",
        "SF:crates/app/src/a.rs\nRG:1,2,3,4\nend_of_record\n",
        "SF:crates/app/src/a.rs\nDA:1,1\n",
    ];
    for text in cases {
        assert!(parse(text).is_err(), "accepted: {text}");
    }
}

#[test]
fn rejects_duplicate_keys_within_one_record() {
    let text = "SF:crates/app/src/a.rs\nDA:1,1\nDA:1,2\nend_of_record\n";
    assert!(parse(text).unwrap_err().contains("duplicate DA"));
}
