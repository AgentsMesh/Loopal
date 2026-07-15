use crate::git_types::{GitChange, GitStatus, RawWorktree};

pub(crate) fn status(bytes: &[u8]) -> GitStatus {
    let mut records = bytes
        .split(|byte| *byte == 0)
        .filter(|item| !item.is_empty());
    let mut result = GitStatus {
        branch: None,
        ahead: 0,
        behind: 0,
        changes: Vec::new(),
    };
    while let Some(record) = records.next() {
        let text = String::from_utf8_lossy(record);
        if let Some(header) = text.strip_prefix("## ") {
            parse_branch(header, &mut result);
            continue;
        }
        if text.len() < 3 {
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        if matches!(chars[0], 'R' | 'C') || matches!(chars[1], 'R' | 'C') {
            let _ = records.next();
        }
        result.changes.push(GitChange {
            path: text[3..].to_string(),
            index_status: chars[0].to_string(),
            worktree_status: chars[1].to_string(),
        });
    }
    result
}

fn parse_branch(header: &str, result: &mut GitStatus) {
    let (head, tracking) = header.split_once("...").unwrap_or((header, ""));
    result.branch = (head != "HEAD (no branch)").then(|| head.to_string());
    let detail = tracking
        .split_once(" [")
        .map(|(_, detail)| detail)
        .unwrap_or("");
    for item in detail.trim_end_matches(']').split(", ") {
        if let Some(value) = item.strip_prefix("ahead ") {
            result.ahead = value.parse().unwrap_or(0);
        } else if let Some(value) = item.strip_prefix("behind ") {
            result.behind = value.parse().unwrap_or(0);
        }
    }
}

pub(crate) fn worktrees(text: &str) -> Vec<RawWorktree> {
    text.split("\n\n")
        .filter_map(|block| {
            let mut item = RawWorktree {
                path: String::new(),
                head: None,
                branch: None,
            };
            for line in block.lines() {
                if let Some(value) = line.strip_prefix("worktree ") {
                    item.path = value.to_string();
                } else if let Some(value) = line.strip_prefix("HEAD ") {
                    item.head = Some(value.to_string());
                } else if let Some(value) = line.strip_prefix("branch refs/heads/") {
                    item.branch = Some(value.to_string());
                }
            }
            (!item.path.is_empty()).then_some(item)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_and_rename() {
        let parsed = status(b"## main...origin/main [ahead 2, behind 1]\0R  new.rs\0old.rs\0");
        assert_eq!(parsed.branch.as_deref(), Some("main"));
        assert_eq!(parsed.ahead, 2);
        assert_eq!(parsed.changes[0].path, "new.rs");
    }
}
