use loopal_tool_api::{Backend, GrepOptions};
use regex::RegexBuilder;

use crate::types::{SearchMatch, SearchParams, SearchResult};
use crate::{WorkspaceError, WorkspaceService};

const MAX_RESULTS: usize = 2_000;
const MAX_PREVIEW_BYTES: usize = 4_000;

impl WorkspaceService {
    pub async fn search(&self, input: SearchParams) -> Result<SearchResult, WorkspaceError> {
        self.require_workspace(&input.workspace_id)?;
        if input.query.is_empty() || input.query.len() > 1_000 {
            return Err(WorkspaceError::invalid("query must contain 1-1000 bytes"));
        }
        let limit = input.max_results.clamp(1, MAX_RESULTS);
        let matcher = RegexBuilder::new(&input.query)
            .build()
            .map_err(|error| WorkspaceError::invalid(format!("invalid query: {error}")))?;
        let result = self
            .backend
            .grep(&GrepOptions {
                pattern: input.query,
                path: None,
                glob_filter: input.glob,
                case_insensitive: false,
                multiline: false,
                fixed_strings: false,
                context_before: 0,
                context_after: 0,
                type_filter: None,
                max_matches: limit.saturating_add(1),
            })
            .await
            .map_err(WorkspaceError::io)?;
        let mut matches = Vec::new();
        let mut preview_truncated = false;
        for file in result.file_matches {
            let path = self.guard.relative(std::path::Path::new(&file.path))?;
            for line in file.groups.into_iter().flat_map(|group| group.lines) {
                if !line.is_match || matches.len() >= limit {
                    continue;
                }
                let column = matcher
                    .find(&line.content)
                    .map(|hit| line.content[..hit.start()].chars().count() + 1)
                    .unwrap_or(1);
                let (preview, was_truncated) = preview(line.content);
                preview_truncated |= was_truncated;
                matches.push(SearchMatch {
                    path: path.clone(),
                    line: line.line_num,
                    column,
                    preview,
                });
            }
        }
        Ok(SearchResult {
            truncated: result.total_match_count > matches.len()
                || result.timed_out
                || preview_truncated,
            matches,
        })
    }
}

fn preview(mut value: String) -> (String, bool) {
    if value.len() <= MAX_PREVIEW_BYTES {
        return (value, false);
    }
    let mut end = MAX_PREVIEW_BYTES - '…'.len_utf8();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push('…');
    (value, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_utf8_safe_and_bounded() {
        let (value, truncated) = preview("界".repeat(2_000));
        assert!(truncated);
        assert!(value.len() <= MAX_PREVIEW_BYTES);
        assert!(value.ends_with('…'));
    }
}
