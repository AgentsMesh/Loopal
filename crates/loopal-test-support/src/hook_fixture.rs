//! Hook script fixture for pre/post tool hook testing.

use std::path::{Path, PathBuf};

/// RAII fixture that creates hook shell scripts in a tempdir.
pub struct HookFixture {
    dir: tempfile::TempDir,
    counter: u32,
}

impl HookFixture {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("hook fixture tempdir"),
            counter: 0,
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Create an echo hook that writes output to a marker file.
    /// Returns (script_path, marker_path).
    pub fn create_echo_hook(&mut self, output: &str) -> (PathBuf, PathBuf) {
        self.counter += 1;
        let script = self.dir.path().join(format!("hook_{}.sh", self.counter));
        let marker = self.dir.path().join(format!("marker_{}.txt", self.counter));
        let content = format!("#!/bin/sh\necho '{}' > '{}'\n", output, marker.display());
        std::fs::write(&script, content).expect("write hook script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).ok();
        }
        (script, marker)
    }

    /// Create a hook that exits with code 1.
    pub fn create_failing_hook(&mut self) -> PathBuf {
        self.counter += 1;
        let script = self.dir.path().join(format!("hook_{}.sh", self.counter));
        std::fs::write(&script, "#!/bin/sh\nexit 1\n").expect("write failing hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).ok();
        }
        script
    }
}

impl Default for HookFixture {
    fn default() -> Self {
        Self::new()
    }
}
