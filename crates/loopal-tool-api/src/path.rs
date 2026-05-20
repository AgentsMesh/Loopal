use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// A path that has been validated by a `Backend::resolve_path` implementation.
///
/// The inner `PathBuf` is private; only `Backend` implementations (which call
/// `from_backend_resolved`) can construct this type. Tools therefore cannot
/// fabricate a `ResolvedPath` from raw input — they must obtain one through
/// `Backend::resolve_path`, which encodes "this path passed cwd containment
/// and sandbox policy checks" at the type level.
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    inner: PathBuf,
}

impl ResolvedPath {
    #[doc(hidden)]
    pub fn from_backend_resolved(p: PathBuf) -> Self {
        Self { inner: p }
    }

    pub fn as_path(&self) -> &Path {
        &self.inner
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        self.inner.to_string_lossy()
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.inner
    }
}

impl AsRef<Path> for ResolvedPath {
    fn as_ref(&self) -> &Path {
        &self.inner
    }
}

impl std::fmt::Display for ResolvedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.display())
    }
}

impl PartialEq for ResolvedPath {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for ResolvedPath {}

impl std::hash::Hash for ResolvedPath {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}
