#[path = "gate/critical.rs"]
mod critical;

use crate::model::{Counter, Coverage, Manifest};

pub const LINE_MIN_EXCLUSIVE: f64 = 95.0;
pub const FUNCTION_MIN_EXCLUSIVE: f64 = 95.0;
pub const REGION_MIN_EXCLUSIVE: f64 = 95.0;
pub const BRANCH_MIN_INCLUSIVE: f64 = 90.0;
pub const PER_FILE_LINE_MIN_INCLUSIVE: f64 = 90.0;

#[derive(Debug)]
pub struct Report {
    pub lines: Counter,
    pub functions: Counter,
    pub branches: Counter,
    pub regions: Counter,
    pub explicit_region_files: usize,
    pub line_proxy_region_files: usize,
}

pub fn apply_branch_supplements(
    coverage: &mut Coverage,
    branches: &Coverage,
    manifest: &Manifest,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for path in &manifest.sources {
        // LLVM omits BRF:0 for branchless files, but a successful export still
        // has DA/FN data. A bare SF is the baseline left by a failed collector.
        match (coverage.files.get_mut(path), branches.files.get(path)) {
            (Some(_), Some(branch_file))
                if branch_file.branches.is_empty()
                    && branch_file.lines.is_empty()
                    && branch_file.functions.is_empty() =>
            {
                errors.push(format!(
                    "branch LCOV has only an SF baseline for included source: {path}"
                ));
            }
            (Some(file), Some(branch_file)) => file.branches.clone_from(&branch_file.branches),
            (None, _) => errors.push(format!("base LCOV is missing included source: {path}")),
            (_, None) => errors.push(format!("branch LCOV is missing included source: {path}")),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn evaluate(coverage: &Coverage, manifest: &Manifest) -> Result<Report, Vec<String>> {
    let mut errors = Vec::new();
    for path in &manifest.sources {
        if !coverage.files.contains_key(path) {
            errors.push(format!("included source is missing from LCOV: {path}"));
        }
    }
    let mut report = Report {
        lines: Counter::default(),
        functions: Counter::default(),
        branches: Counter::default(),
        regions: Counter::default(),
        explicit_region_files: 0,
        line_proxy_region_files: 0,
    };
    for (path, file) in &coverage.files {
        if file.lines.is_empty() && file.functions.is_empty() {
            errors.push(format!("scoped source has no DA/FN data: {path}"));
        }
        add(&mut report.lines, file.line_counter());
        add(&mut report.functions, file.function_counter());
        add(&mut report.branches, file.branch_counter());
        let regions = if file.explicit_regions {
            report.explicit_region_files += 1;
            file.explicit_region_counter()
        } else {
            report.line_proxy_region_files += 1;
            file.line_counter()
        };
        add(&mut report.regions, regions);
        let lines = file.line_counter();
        if lines.percent() < PER_FILE_LINE_MIN_INCLUSIVE {
            errors.push(format!(
                "per-file line coverage for {path} is {:.2}% ({}/{}) < 90.00%",
                lines.percent(),
                lines.hit,
                lines.found
            ));
        }
    }
    threshold(&mut errors, "line", &report.lines, LINE_MIN_EXCLUSIVE, true);
    threshold(
        &mut errors,
        "function",
        &report.functions,
        FUNCTION_MIN_EXCLUSIVE,
        true,
    );
    threshold(
        &mut errors,
        "region",
        &report.regions,
        REGION_MIN_EXCLUSIVE,
        true,
    );
    threshold(
        &mut errors,
        "branch",
        &report.branches,
        BRANCH_MIN_INCLUSIVE,
        false,
    );
    for critical in &manifest.critical {
        let hits = coverage
            .files
            .get(&critical.path)
            .and_then(|file| critical::hits(&file.functions, &file.function_lines, &critical.name));
        match hits {
            Some(0) => errors.push(format!(
                "critical function is uncovered: {}|{}",
                critical.path, critical.name
            )),
            None => errors.push(format!(
                "critical function is missing from LCOV: {}|{}",
                critical.path, critical.name
            )),
            Some(_) => {}
        }
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}

fn threshold(errors: &mut Vec<String>, name: &str, value: &Counter, minimum: f64, exclusive: bool) {
    if value.found == 0 {
        errors.push(format!("{name} coverage has no scoped data"));
        return;
    }
    let failed = if exclusive {
        value.percent() <= minimum
    } else {
        value.percent() < minimum
    };
    if failed {
        let operator = if exclusive { ">" } else { ">=" };
        errors.push(format!(
            "global {name} coverage is {:.2}% ({}/{}) but must be {operator} {minimum:.2}%",
            value.percent(),
            value.hit,
            value.found
        ));
    }
}

fn add(target: &mut Counter, value: Counter) {
    target.found += value.found;
    target.hit += value.hit;
}
