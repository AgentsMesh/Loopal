//! Plan file management — slug generation, path resolution, read/write.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::warn;

/// Manages a session-scoped plan file.
///
/// The plan file lives at `{cwd}/.loopal/plans/{slug}.md`.
/// Sub-agents get `{slug}-agent-{name}.md` to avoid collisions.
pub struct PlanFile {
    path: PathBuf,
    cwd: PathBuf,
}

impl PlanFile {
    /// Create a new plan file for the main agent.
    /// Retries slug generation to avoid collision with existing files.
    pub fn new(cwd: &Path) -> Self {
        let dir = plans_dir(cwd);
        for attempt in 0..10u32 {
            let slug = generate_slug_with_attempt(attempt);
            let path = dir.join(format!("{slug}.md"));
            if !path.exists() {
                return Self {
                    path,
                    cwd: cwd.to_path_buf(),
                };
            }
        }
        let slug = generate_slug_with_attempt(99);
        let path = dir.join(format!("{slug}.md"));
        Self {
            path,
            cwd: cwd.to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    /// Read plan content from disk. Returns `None` if missing or unreadable.
    pub fn read(&self) -> Option<String> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) if !s.trim().is_empty() => Some(s),
            Ok(_) => None, // empty file treated as absent
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                warn!(path = %self.path.display(), error = %e, "failed to read plan file");
                None
            }
        }
    }

    /// Check if a given path matches the plan file path.
    pub fn matches_path(&self, candidate: &str) -> bool {
        let candidate = Path::new(candidate);
        // Resolve relative paths against cwd (LLM often passes relative paths).
        let abs_candidate = if candidate.is_relative() {
            self.cwd.join(candidate)
        } else {
            candidate.to_path_buf()
        };
        // Both paths exist: compare canonical forms.
        if let (Ok(a), Ok(b)) = (abs_candidate.canonicalize(), self.path.canonicalize()) {
            return a == b;
        }
        // Plan file doesn't exist yet (first write): normalize and compare.
        let norm_cand = normalize(&abs_candidate);
        let norm_plan = normalize(&self.path);
        norm_cand == norm_plan
    }
}

/// Normalize a path by resolving `.` and `..` without requiring the file to exist.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Return the plans directory path. Creates it lazily only when needed.
fn plans_dir(cwd: &Path) -> PathBuf {
    cwd.join(".loopal").join("plans")
}

/// Generate a short readable slug (adjective-noun-noun pattern).
/// `attempt` is mixed into the seed to ensure different results on retry.
fn generate_slug_with_attempt(attempt: u32) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let time_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Mix in process id + attempt to avoid collisions across retries and
    // concurrent processes.
    let seed = time_seed ^ ((std::process::id() as u128) << 32) ^ (attempt as u128 * 9973);

    let adjectives = [
        "calm", "bold", "warm", "pure", "keen", "soft", "wise", "fair", "swift", "bright", "cool",
        "clear", "fresh", "light", "grand",
    ];
    let nouns = [
        "brook", "ridge", "grove", "stone", "cliff", "trail", "coast", "field", "lake", "cedar",
        "hawk", "crane", "dawn", "frost", "bloom",
    ];
    let a = adjectives[(seed as usize) % adjectives.len()];
    let n1 = nouns[((seed / 17) as usize) % nouns.len()];
    let n2 = nouns[((seed / 257) as usize) % nouns.len()];
    format!("{a}-{n1}-{n2}")
}

/// Append a `<system-reminder>` to tool_result content during plan mode.
pub fn wrap_plan_reminder(content: &str, plan_path: &str) -> String {
    format!(
        "{content}\n\n<system-reminder>\n\
         Plan mode still active (see full instructions earlier in conversation). \
         Read-only except plan file ({plan_path}). Follow 5-phase workflow. \
         End turns with AskUser (for clarifications) or ExitPlanMode \
         (for plan approval). Never ask about plan approval via text or \
         AskUser.\n</system-reminder>"
    )
}

/// Build the set of tool names allowed in plan mode.
///
/// Includes: all ReadOnly tools + Write + Edit (path-checked separately) +
/// special tools (EnterPlanMode, ExitPlanMode, AskUser, Agent).
pub fn build_plan_mode_filter(kernel: &loopal_kernel::Kernel) -> HashSet<String> {
    use loopal_tool_api::PermissionLevel;
    let mut allowed = HashSet::new();
    for def in kernel.tool_definitions() {
        if let Some(tool) = kernel.get_tool(&def.name)
            && tool.permission() == PermissionLevel::ReadOnly
        {
            allowed.insert(def.name);
        }
    }
    // Write/Edit allowed but path-restricted to plan file (checked in tools_check).
    allowed.insert("Write".into());
    allowed.insert("Edit".into());
    // Special intercepted tools always available.
    allowed.insert("EnterPlanMode".into());
    allowed.insert("ExitPlanMode".into());
    allowed.insert("AskUser".into());
    allowed.insert("Agent".into());
    allowed
}
