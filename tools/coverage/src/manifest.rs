use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::model::{CriticalFunction, Manifest};

pub fn load(sources: &Path, critical: &Path) -> Result<Manifest, String> {
    let sources = parse_lines(sources)?;
    if sources.is_empty() {
        return Err("included-source manifest is empty".into());
    }
    let mut source_set = BTreeSet::new();
    for value in sources {
        validate_path(&value)?;
        if !source_set.insert(value.clone()) {
            return Err(format!("duplicate included source: {value}"));
        }
    }
    let mut critical_functions = Vec::new();
    for row in parse_lines(critical)? {
        let (path, name) = row
            .split_once('|')
            .ok_or_else(|| format!("malformed critical function row: {row}"))?;
        validate_path(path)?;
        if name.trim().is_empty() || name != name.trim() {
            return Err(format!("invalid critical function name: {name:?}"));
        }
        if !source_set.contains(path) {
            return Err(format!("critical function source is not included: {path}"));
        }
        critical_functions.push(CriticalFunction {
            path: path.into(),
            name: name.into(),
        });
    }
    if critical_functions.is_empty() {
        return Err("critical-function manifest is empty".into());
    }
    Ok(Manifest {
        sources: source_set,
        critical: critical_functions,
    })
}

fn parse_lines(path: &Path) -> Result<Vec<String>, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.trim() != path
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !path.ends_with(".rs")
    {
        return Err(format!(
            "invalid workspace-relative Rust source path: {path:?}"
        ));
    }
    Ok(())
}
