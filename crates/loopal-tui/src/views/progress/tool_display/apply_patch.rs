use ratatui::prelude::*;

use loopal_edit_core::patch_parser::parse_patch;
use loopal_edit_core::patch_types::{FileOp, HunkLine};
use loopal_view_state::ToolInvocation;

use super::diff_style::{self, DIFF_MAX_LINES};
use super::output_first_line;

pub fn extract_detail(input: &serde_json::Value) -> Option<String> {
    let patch = input.get("patch")?.as_str()?;
    let ops = parse_patch(patch).ok()?;
    Some(format!("{} file(s)", ops.len()))
}

pub fn render_body(tc: &ToolInvocation) -> Vec<Line<'static>> {
    let Some(patch) = tc
        .input
        .as_ref()
        .and_then(|i| i.get("patch"))
        .and_then(|v| v.as_str())
    else {
        return vec![output_first_line("patch applied")];
    };

    let Ok(ops) = parse_patch(patch) else {
        return vec![output_first_line("patch applied")];
    };

    let stats = patch_stats(&ops);
    let mut diff_lines: Vec<Line<'static>> = Vec::new();
    let mut shown = 0usize;

    for op in &ops {
        push_header(&mut diff_lines, &mut shown, op);
        match op {
            FileOp::Add { content, .. } => {
                for line in content.lines() {
                    push_added(&mut diff_lines, &mut shown, line);
                }
            }
            FileOp::Update { hunks, .. } => {
                for line in hunks.iter().flat_map(|hunk| &hunk.lines) {
                    match line {
                        HunkLine::Add(text) => push_added(&mut diff_lines, &mut shown, text),
                        HunkLine::Remove(text) => push_removed(&mut diff_lines, &mut shown, text),
                        HunkLine::Context(_) => {}
                    }
                }
            }
            FileOp::Delete { .. } => {}
        }
    }

    let summary = format_patch_summary(stats.files, stats.added, stats.removed);
    let mut lines = vec![output_first_line(&summary)];
    lines.extend(diff_lines);

    let total = stats.added + stats.removed + stats.files;
    if total > DIFF_MAX_LINES {
        lines.push(diff_style::fold_indicator(total - DIFF_MAX_LINES));
    }
    lines
}

#[derive(Debug, Default, PartialEq, Eq)]
struct PatchStats {
    files: usize,
    added: usize,
    removed: usize,
}

fn patch_stats(ops: &[FileOp]) -> PatchStats {
    let mut stats = PatchStats {
        files: ops.len(),
        ..PatchStats::default()
    };
    for op in ops {
        match op {
            FileOp::Add { content, .. } => stats.added += content.lines().count(),
            FileOp::Update { hunks, .. } => {
                for line in hunks.iter().flat_map(|hunk| &hunk.lines) {
                    match line {
                        HunkLine::Add(_) => stats.added += 1,
                        HunkLine::Remove(_) => stats.removed += 1,
                        HunkLine::Context(_) => {}
                    }
                }
            }
            FileOp::Delete { .. } => {}
        }
    }
    stats
}

fn push_header(lines: &mut Vec<Line<'static>>, shown: &mut usize, op: &FileOp) {
    let header = match op {
        FileOp::Add { path, .. } => format!("*** Add File: {}", path.display()),
        FileOp::Update { path, .. } => format!("*** Update File: {}", path.display()),
        FileOp::Delete { path } => format!("*** Delete File: {}", path.display()),
    };
    if *shown < DIFF_MAX_LINES {
        lines.push(Line::from(Span::styled(
            format!("    {header}"),
            diff_style::header_style(),
        )));
        *shown += 1;
    }
}

fn push_added(lines: &mut Vec<Line<'static>>, shown: &mut usize, text: &str) {
    if *shown < DIFF_MAX_LINES {
        lines.push(diff_style::added_line(text));
        *shown += 1;
    }
}

fn push_removed(lines: &mut Vec<Line<'static>>, shown: &mut usize, text: &str) {
    if *shown < DIFF_MAX_LINES {
        lines.push(diff_style::removed_line(text));
        *shown += 1;
    }
}

fn format_patch_summary(files: usize, added: usize, removed: usize) -> String {
    let mut parts = vec![format!("{files} file(s)")];
    if added > 0 {
        parts.push(format!("+{added}"));
    }
    if removed > 0 {
        parts.push(format!("-{removed}"));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ENVELOPED_PATCH: &str = "\
*** Begin Patch
*** Add File: new.txt
+first
+second
*** Update File: old.txt
@@
-before
+after
 unchanged
*** Delete File: gone.txt
*** End Patch
";

    #[test]
    fn detail_counts_operations_not_envelope_markers() {
        let detail = extract_detail(&json!({ "patch": ENVELOPED_PATCH }));
        assert_eq!(detail.as_deref(), Some("3 file(s)"));
    }

    #[test]
    fn stats_are_derived_from_parsed_operations() {
        let ops = parse_patch(ENVELOPED_PATCH).unwrap();
        assert_eq!(
            patch_stats(&ops),
            PatchStats {
                files: 3,
                added: 3,
                removed: 1,
            }
        );
    }

    #[test]
    fn invalid_patch_has_no_misleading_file_count() {
        let detail = extract_detail(&json!({
            "patch": "*** Begin Patch\n*** Add File: file.txt\n+content\n"
        }));
        assert_eq!(detail, None);
    }
}
