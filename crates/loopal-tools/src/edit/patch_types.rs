use std::path::PathBuf;

/// A parsed file operation from a patch.
#[derive(Debug)]
pub enum FileOp {
    Add { path: PathBuf, content: String },
    Update { path: PathBuf, hunks: Vec<Hunk> },
    Delete { path: PathBuf },
}

impl FileOp {
    pub fn path(&self) -> &PathBuf {
        match self {
            Self::Add { path, .. } | Self::Update { path, .. } | Self::Delete { path } => path,
        }
    }
}

/// A hunk within an Update operation.
#[derive(Debug)]
pub struct Hunk {
    pub line_hint: Option<usize>,
    pub lines: Vec<HunkLine>,
}

/// A single line within a hunk.
#[derive(Debug)]
pub enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

/// Instruction to write (or delete) a file after applying a patch.
pub struct FileWrite {
    pub path: PathBuf,
    /// `None` means delete the file.
    pub content: Option<String>,
}
