//! Shared file-walker configuration for glob and grep searches.

use std::path::Path;

use ignore::WalkBuilder;
use ignore::types::TypesBuilder;

/// Build a `WalkBuilder` with shared defaults.
///
/// * Does not follow symlinks (ripgrep default) — avoids cross-mount escape and traversal cycles.
/// * Respects `.gitignore` (ignore crate default).
/// * Applies file-type filtering when `type_filter` is given.
///
/// Returns `None` when `type_filter` names an unknown file type — the
/// caller should short-circuit with an empty result.
pub fn build_walker(search_path: &Path, type_filter: Option<&str>) -> Option<WalkBuilder> {
    let mut builder = WalkBuilder::new(search_path);
    builder.follow_links(false);

    if let Some(ty) = type_filter {
        let mut tb = TypesBuilder::new();
        tb.add_defaults();
        tb.select(ty);
        match tb.build() {
            Ok(types) => builder.types(types),
            Err(_) => return None, // Unknown type → zero results.
        };
    }

    Some(builder)
}
