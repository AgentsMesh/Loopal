use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Resolve a file-valued environment variable from Bazel runfiles.
///
/// A configured value is authoritative: callers receive an error when it
/// cannot be resolved. `Ok(None)` is returned only when the variable is absent.
pub fn resolve_runfile_env(variable: &str) -> anyhow::Result<Option<PathBuf>> {
    let Some(configured) = std::env::var_os(variable) else {
        return Ok(None);
    };
    resolve_configured_file(variable, &configured, &RunfilesLocations::from_env()).map(Some)
}

/// Resolve a required file-valued environment variable from Bazel runfiles.
pub fn require_runfile_env(variable: &str) -> anyhow::Result<PathBuf> {
    resolve_runfile_env(variable)?
        .ok_or_else(|| anyhow::anyhow!("{variable} must be set to a file path"))
}

#[derive(Default)]
struct RunfilesLocations {
    runfiles_dir: Option<PathBuf>,
    test_srcdir: Option<PathBuf>,
    manifest: Option<PathBuf>,
}

impl RunfilesLocations {
    fn from_env() -> Self {
        Self {
            runfiles_dir: std::env::var_os("RUNFILES_DIR").map(PathBuf::from),
            test_srcdir: std::env::var_os("TEST_SRCDIR").map(PathBuf::from),
            manifest: std::env::var_os("RUNFILES_MANIFEST_FILE").map(PathBuf::from),
        }
    }
}

fn resolve_configured_file(
    variable: &str,
    configured: &OsStr,
    locations: &RunfilesLocations,
) -> anyhow::Result<PathBuf> {
    let logical = PathBuf::from(configured);
    if logical.as_os_str().is_empty() {
        return Err(unresolved(variable, &logical));
    }
    if logical.is_absolute() {
        return existing_file(variable, &logical, &logical);
    }
    if logical.is_file() {
        return canonicalize(variable, &logical, &logical);
    }
    for root in [&locations.runfiles_dir, &locations.test_srcdir]
        .into_iter()
        .flatten()
    {
        let candidate = root.join(&logical);
        if candidate.is_file() {
            return canonicalize(variable, &logical, &candidate);
        }
    }
    if let (Some(manifest), Some(key)) = (&locations.manifest, logical.to_str())
        && let Some(candidate) = manifest_entry(manifest, key).map_err(|error| {
            anyhow::anyhow!(
                "failed to resolve {variable} runfile '{}': could not read manifest '{}': {error}",
                logical.display(),
                manifest.display()
            )
        })?
    {
        return existing_file(variable, &logical, &candidate);
    }
    Err(unresolved(variable, &logical))
}

fn manifest_entry(manifest: &Path, key: &str) -> std::io::Result<Option<PathBuf>> {
    let contents = std::fs::read_to_string(manifest)?;
    Ok(contents.lines().find_map(|line| {
        let (logical, physical) = parse_manifest_entry(line.trim_end_matches('\r'))?;
        (logical == key).then(|| PathBuf::from(physical))
    }))
}

fn parse_manifest_entry(line: &str) -> Option<(String, String)> {
    let (escaped, entry) = match line.strip_prefix(' ') {
        Some(entry) => (true, entry),
        None => (false, line),
    };
    let (logical, physical) = entry.split_once(' ')?;
    if !escaped {
        return Some((logical.to_owned(), physical.to_owned()));
    }
    Some((
        decode_manifest_field(logical),
        decode_manifest_field(physical),
    ))
}

fn decode_manifest_field(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match chars.next() {
            Some('s') => decoded.push(' '),
            Some('n') => decoded.push('\n'),
            Some('b') => decoded.push('\\'),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }
    decoded
}

fn existing_file(variable: &str, logical: &Path, candidate: &Path) -> anyhow::Result<PathBuf> {
    if !candidate.is_file() {
        return Err(unresolved(variable, logical));
    }
    canonicalize(variable, logical, candidate)
}

fn canonicalize(variable: &str, logical: &Path, candidate: &Path) -> anyhow::Result<PathBuf> {
    std::fs::canonicalize(candidate).map_err(|error| {
        anyhow::anyhow!(
            "failed to resolve {variable} runfile '{}': {error}",
            logical.display()
        )
    })
}

fn unresolved(variable: &str, logical: &Path) -> anyhow::Error {
    anyhow::anyhow!(
        "failed to resolve {variable} runfile '{}': file not found",
        logical.display()
    )
}

#[cfg(test)]
#[path = "runfiles/tests.rs"]
mod tests;
