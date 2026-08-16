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
    let path = fixture(text);
    let result = lcov::parse(&path, Path::new("/workspace/Loopal"), sources).unwrap();
    fs::remove_file(path).ok();
    result
}

fn fixture(text: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "loopal-gate-{}-{}.lcov",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, text).unwrap();
    path
}

fn record(path: &str, regions: Option<usize>) -> String {
    let mut out = format!("SF:{path}\n");
    for line in 1..=100 {
        out += &format!("DA:{line},{}\n", u8::from(line > 4));
    }
    let name = if path == A { "critical" } else { "helper" };
    out += &format!("FN:1,{name}\nFNDA:1,{name}\n");
    for branch in 0..10 {
        out += &format!("BRDA:1,0,{branch},{}\n", u8::from(branch > 0));
    }
    if let Some(total) = regions {
        for region in 1..=total {
            out += &format!("RG:{region},1,{region},2,{}\n", u8::from(region > 4));
        }
    }
    out + "end_of_record\n"
}

#[test]
fn passing_explicit_region_data() {
    let sources = manifest(&[A]);
    let report = gate::evaluate(&parse(&record(A, Some(100)), &sources), &sources).unwrap();
    assert_eq!(report.explicit_region_files, 1);
    assert_eq!(report.line_proxy_region_files, 0);
}

#[test]
fn ordinary_lcov_gates_regions_with_lines() {
    let sources = manifest(&[A]);
    let report = gate::evaluate(&parse(&record(A, None), &sources), &sources).unwrap();
    assert_eq!(report.line_proxy_region_files, 1);
    assert_eq!(report.lines, report.regions);
}

#[test]
fn mixed_region_policy_is_per_file() {
    let sources = manifest(&[A, B]);
    let text = record(A, Some(100)) + &record(B, None);
    let report = gate::evaluate(&parse(&text, &sources), &sources).unwrap();
    assert_eq!(report.explicit_region_files, 1);
    assert_eq!(report.line_proxy_region_files, 1);
    assert_eq!(report.regions.found, 200);
    assert_eq!(report.regions.hit, 192);
}
