#[path = "../src/gate.rs"]
mod gate;
#[path = "../src/lcov.rs"]
mod lcov;
#[path = "../src/manifest.rs"]
mod manifest;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/path.rs"]
mod path;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use model::{CriticalFunction, Manifest};

static NEXT: AtomicU64 = AtomicU64::new(0);
const A: &str = "crates/app/src/a.rs";
const B: &str = "crates/app/src/b.rs";

fn manifest(paths: &[&str]) -> Manifest {
    Manifest {
        sources: paths.iter().map(|path| (*path).into()).collect(),
        critical: vec![CriticalFunction {
            path: A.into(),
            name: "critical".into(),
        }],
    }
}

fn parse(text: &str, sources: &Manifest) -> model::Coverage {
    let file = fixture(text);
    let result = lcov::parse(&file, Path::new("/workspace/Loopal"), sources).unwrap();
    fs::remove_file(file).ok();
    result
}

fn fixture(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal-threshold-{}-{}.lcov",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, text).unwrap();
    path
}

fn record(
    path: &str,
    lines: &[u64],
    functions: &[(&str, u64)],
    branches: &[u64],
    regions: Option<&[u64]>,
) -> String {
    let mut out = format!("SF:{path}\n");
    for (line, hits) in lines.iter().enumerate() {
        out += &format!("DA:{},{}\n", line + 1, hits);
    }
    for (index, (name, hits)) in functions.iter().enumerate() {
        out += &format!("FN:{},{name}\nFNDA:{hits},{name}\n", index + 1);
    }
    for (branch, hits) in branches.iter().enumerate() {
        out += &format!("BRDA:1,0,{branch},{hits}\n");
    }
    if let Some(regions) = regions {
        for (i, hits) in regions.iter().enumerate() {
            out += &format!("RG:{},1,{},2,{hits}\n", i + 1, i + 1);
        }
    }
    out + "end_of_record\n"
}

fn hits(total: usize, missed: usize) -> Vec<u64> {
    (0..total).map(|i| u64::from(i >= missed)).collect()
}

fn failures(text: &str) -> Vec<String> {
    let sources = manifest(&[A]);
    gate::evaluate(&parse(text, &sources), &sources).unwrap_err()
}

#[test]
fn each_global_threshold_fails() {
    let line = record(A, &hits(20, 1), &[("critical", 1)], &hits(10, 1), None);
    assert!(failures(&line).iter().any(|e| e.contains("global line")));
    let function = record(
        A,
        &hits(100, 4),
        &[("critical", 1), ("miss", 0)],
        &hits(10, 1),
        None,
    );
    assert!(
        failures(&function)
            .iter()
            .any(|e| e.contains("global function"))
    );
    let branch = record(A, &hits(100, 4), &[("critical", 1)], &hits(10, 2), None);
    assert!(
        failures(&branch)
            .iter()
            .any(|e| e.contains("global branch"))
    );
    let region = record(
        A,
        &hits(100, 4),
        &[("critical", 1)],
        &hits(10, 1),
        Some(&hits(20, 1)),
    );
    assert!(
        failures(&region)
            .iter()
            .any(|e| e.contains("global region"))
    );
}

#[test]
fn per_file_line_failure_is_independent() {
    let mut text = record(A, &hits(11, 2), &[("critical", 1)], &hits(10, 1), None);
    text += &record(B, &hits(200, 0), &[("other", 1)], &hits(10, 1), None);
    let sources = manifest(&[A, B]);
    let failures = gate::evaluate(&parse(&text, &sources), &sources).unwrap_err();
    assert!(
        failures
            .iter()
            .any(|e| e.contains("per-file line coverage"))
    );
    assert!(
        !failures
            .iter()
            .any(|e| e.starts_with("global line coverage"))
    );
}

#[path = "threshold_rust_symbols.rs"]
mod rust_symbols;

#[test]
fn critical_function_failures_are_named() {
    let uncovered = record(A, &hits(100, 4), &[("critical", 0)], &hits(10, 1), None);
    assert!(
        failures(&uncovered)
            .iter()
            .any(|e| e.contains("critical function is uncovered"))
    );
    let missing = record(A, &hits(100, 4), &[("other", 1)], &hits(10, 1), None);
    assert!(
        failures(&missing)
            .iter()
            .any(|e| e.contains("critical function is missing"))
    );
}
