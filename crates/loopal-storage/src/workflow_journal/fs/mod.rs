use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};

use super::WorkflowJournalError;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[derive(Clone, Debug)]
pub(crate) struct JournalLocation {
    base_dir: PathBuf,
    session_id: String,
    file_name: OsString,
    display_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileIdentity(platform::FileIdentity);

pub(crate) struct OpenedJournal {
    pub(crate) file: File,
    pub(crate) identity: FileIdentity,
}

pub(crate) struct DiscoveredJournal {
    pub(crate) name: OsString,
    pub(crate) bytes: u64,
    pub(crate) identity: FileIdentity,
}

#[derive(Clone, Copy)]
pub(crate) enum OpenMode {
    Read,
    AppendCreate,
    AppendExisting,
    Repair,
}

pub(super) enum FsError {
    Missing,
    Integrity(&'static str),
    Io(std::io::Error),
}

impl JournalLocation {
    pub(crate) fn new(
        base_dir: &Path,
        session_id: &str,
        run_id: &str,
        display_path: PathBuf,
    ) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            session_id: session_id.to_owned(),
            file_name: format!("{run_id}.jsonl").into(),
            display_path,
        }
    }

    pub(crate) fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }
}

pub(crate) fn open(
    location: &JournalLocation,
    mode: OpenMode,
) -> Result<Option<OpenedJournal>, WorkflowJournalError> {
    match platform::open(location, mode) {
        Ok(opened) => Ok(Some(OpenedJournal {
            file: opened.file,
            identity: FileIdentity(opened.identity),
        })),
        Err(FsError::Missing) => Ok(None),
        Err(error) => Err(map_error(location.display_path(), error)),
    }
}

pub(crate) fn discover(
    base_dir: &Path,
    session_id: &str,
    display_path: &Path,
) -> Result<Vec<DiscoveredJournal>, WorkflowJournalError> {
    match platform::discover(base_dir, session_id) {
        Ok(entries) => Ok(entries
            .into_iter()
            .map(|entry| DiscoveredJournal {
                name: entry.name,
                bytes: entry.bytes,
                identity: FileIdentity(entry.identity),
            })
            .collect()),
        Err(FsError::Missing) => Ok(Vec::new()),
        Err(error) => Err(map_error(display_path, error)),
    }
}

fn map_error(path: &Path, error: FsError) -> WorkflowJournalError {
    match error {
        FsError::Missing => WorkflowJournalError::io(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "workflow journal is missing"),
        ),
        FsError::Integrity(detail) => WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: 0,
            detail: detail.into(),
        },
        FsError::Io(error) => WorkflowJournalError::io(path, error),
    }
}

pub(crate) fn same_identity(left: &FileIdentity, right: &FileIdentity) -> bool {
    left == right
}

pub(crate) fn verify_identity(
    path: &Path,
    expected: Option<&FileIdentity>,
    actual: &FileIdentity,
) -> Result<(), WorkflowJournalError> {
    if expected.is_some_and(|expected| !same_identity(expected, actual)) {
        Err(WorkflowJournalError::Corruption {
            path: path.to_path_buf(),
            offset: 0,
            detail: "workflow journal identity changed after discovery".into(),
        })
    } else {
        Ok(())
    }
}

pub(super) fn parts(location: &JournalLocation) -> (&Path, &str, &std::ffi::OsStr) {
    (
        &location.base_dir,
        &location.session_id,
        &location.file_name,
    )
}
