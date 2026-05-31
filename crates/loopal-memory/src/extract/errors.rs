use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ExtractionError {
    #[error("frontmatter parse failed: {0}")]
    FrontmatterParse(String),

    #[error("file contains git merge conflict marker — skipping")]
    MergeConflictMarker,

    #[error("missing required field for fallback: {0}")]
    MissingField(String),

    #[error("invalid wikilink at line {line}: {raw}")]
    InvalidWikilink { line: u32, raw: String },
}
