//! RAII test fixture — isolated tempdir with file helpers.

use std::path::{Path, PathBuf};

use chrono::Utc;
use loopal_runtime::SessionManager;
use loopal_storage::Session;

/// Per-test isolated temporary directory. Dropped automatically on test exit.
pub struct TestFixture {
    dir: tempfile::TempDir,
}

impl TestFixture {
    /// Create a new fixture with a unique temporary directory.
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("failed to create test tempdir"),
        }
    }

    /// Root path of the temporary directory.
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Create a file inside the fixture directory and return its absolute path.
    pub fn create_file(&self, relative_path: &str, content: &str) -> PathBuf {
        let path = self.dir.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent dir");
        }
        std::fs::write(&path, content).expect("write fixture file");
        path
    }

    /// Read a file from the fixture directory.
    pub fn read_file(&self, relative_path: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(relative_path)).expect("read fixture file")
    }

    /// Whether a file exists in the fixture directory.
    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.dir.path().join(relative_path).exists()
    }

    /// Build a `Session` rooted in this fixture's tempdir.
    ///
    /// Canonicalizes the path to avoid macOS `/var` → `/private/var` mismatch
    /// with `LocalBackend` which also canonicalizes.
    pub fn test_session(&self, id: &str) -> Session {
        let canonical = self
            .dir
            .path()
            .canonicalize()
            .unwrap_or_else(|_| self.dir.path().to_path_buf());
        // Strip Windows extended path prefix (\\?\) to match LocalBackend behavior
        let cwd_str = canonical.to_string_lossy().into_owned();
        #[cfg(windows)]
        let cwd_str = cwd_str
            .strip_prefix("\\\\?\\")
            .unwrap_or(&cwd_str)
            .to_string();
        Session {
            id: id.into(),
            title: String::new(),
            model: "claude-sonnet-4-20250514".into(),
            cwd: cwd_str,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            mode: "default".into(),
        }
    }

    /// Build a `SessionManager` writing to a subdirectory of this fixture.
    pub fn session_manager(&self) -> SessionManager {
        SessionManager::with_base_dir(self.dir.path().join("sessions"))
    }
}

impl Default for TestFixture {
    fn default() -> Self {
        Self::new()
    }
}
