#[derive(Debug, Clone)]
pub struct MultiEditItem {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Debug)]
pub struct MultiEditOutcome {
    pub content: String,
    pub applied: usize,
}

#[derive(Debug)]
pub enum MultiEditError {
    NotFound { index: usize },
    MultipleMatches { index: usize, count: usize },
}

impl std::fmt::Display for MultiEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { index } => {
                write!(f, "edit {index}: old_string not found in current content")
            }
            Self::MultipleMatches { index, count } => write!(
                f,
                "edit {index}: old_string found {count} times; must be unique"
            ),
        }
    }
}

pub fn apply_multi_edits(
    original: &str,
    edits: &[MultiEditItem],
) -> Result<MultiEditOutcome, MultiEditError> {
    let mut current = original.to_string();
    for (i, edit) in edits.iter().enumerate() {
        let count = current.matches(&edit.old_string).count();
        match count {
            0 => return Err(MultiEditError::NotFound { index: i }),
            1 => {
                current = current.replacen(&edit.old_string, &edit.new_string, 1);
            }
            n => {
                return Err(MultiEditError::MultipleMatches { index: i, count: n });
            }
        }
    }
    Ok(MultiEditOutcome {
        content: current,
        applied: edits.len(),
    })
}
