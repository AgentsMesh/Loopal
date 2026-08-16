mod gate;
mod lcov;
mod manifest;
mod model;
mod path;

use std::env;
use std::path::{Path, PathBuf};

const MAX_REPORTS: usize = 128;

const HELP: &str = r#"Loopal scoped Rust LCOV gate

Usage:
  bazel coverage --combined_report=lcov <curated-targets>
  bazel coverage --config=rust_branch <bounded-branch-shard>
  bazel run //tools/coverage:gate -- BASE.lcov [BRANCH.lcov ...]

The first report is the sole line/function/region source. Additional reports contribute
only branch records; their union must contain every scoped source. At most 128 reports
are accepted. With no argument, BASE defaults to bazel-out/_coverage/_coverage_report.dat.

The checked-in manifest is the only production scope. Test, external, generated, and
unlisted SF records are ignored. Paths from the workspace, absolute workspace paths,
and Bazel execroots are normalized before matching. Duplicate SF records are merged.

Thresholds: global line/function/region >95%, branch >=90%, every file line >=90%,
and every named critical function must be hit. Ordinary LCOV has no LLVM region type:
each scoped file uses RG:start_line,start_col,end_line,end_col,count when it has RG;
files without RG use DA as the deterministic region proxy. Mixed reports aggregate both
policies and print the number of files using each, so the region gate is never skipped.
"#;

fn main() {
    if let Err(error) = run() {
        eprintln!("coverage gate failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{HELP}");
        return Ok(());
    }
    if args.len() > MAX_REPORTS {
        return Err(format!(
            "at most {MAX_REPORTS} LCOV reports are accepted; use --help"
        ));
    }
    let workspace = env::var("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .or_else(|_| env::current_dir())
        .map_err(|error| format!("cannot determine workspace: {error}"))?;
    let runfiles = runfile_root()?;
    let manifest = manifest::load(
        &runfiles.join("tools/coverage/included_sources.txt"),
        &runfiles.join("tools/coverage/critical_functions.txt"),
    )?;
    let inputs: Vec<_> = if args.is_empty() {
        vec![workspace.join("bazel-out/_coverage/_coverage_report.dat")]
    } else {
        args.iter()
            .map(|path| absolute_input(Path::new(path), &workspace))
            .collect()
    };
    let mut coverage = lcov::parse(&inputs[0], &workspace, &manifest)?;
    if inputs.len() > 1 {
        let branches = lcov::parse_many(&inputs[1..], &workspace, &manifest)?;
        gate::apply_branch_supplements(&mut coverage, &branches, &manifest)
            .map_err(|errors| errors.join("\n"))?;
    }
    match gate::evaluate(&coverage, &manifest) {
        Ok(report) => {
            print_counter("line", &report.lines);
            print_counter("function", &report.functions);
            print_counter("branch", &report.branches);
            print_counter("region", &report.regions);
            println!(
                "region policy: per-file RG for {} file(s), DA line proxy for {} file(s)",
                report.explicit_region_files, report.line_proxy_region_files
            );
            println!(
                "coverage gate passed for {} scoped files",
                coverage.files.len()
            );
            Ok(())
        }
        Err(errors) => Err(errors.join("\n")),
    }
}

fn absolute_input(path: &Path, workspace: &Path) -> PathBuf {
    if path.is_absolute() {
        path.into()
    } else {
        workspace.join(path)
    }
}

fn runfile_root() -> Result<PathBuf, String> {
    if let Ok(dir) = env::var("RUNFILES_DIR") {
        let root = PathBuf::from(dir).join("_main");
        if root.exists() {
            return Ok(root);
        }
    }
    if let Ok(manifest) = env::var("RUNFILES_MANIFEST_FILE") {
        let text = std::fs::read_to_string(manifest).map_err(|error| error.to_string())?;
        if let Some((_, physical)) = text.lines().find_map(|line| {
            let (logical, physical) = line.split_once(' ')?;
            (logical == "_main/tools/coverage/included_sources.txt").then_some((logical, physical))
        }) {
            return PathBuf::from(physical)
                .ancestors()
                .nth(3)
                .map(PathBuf::from)
                .ok_or_else(|| "invalid runfiles manifest entry".into());
        }
    }
    env::current_dir().map_err(|error| error.to_string())
}

fn print_counter(name: &str, counter: &model::Counter) {
    println!(
        "{name}: {:.2}% ({}/{})",
        counter.percent(),
        counter.hit,
        counter.found
    );
}
