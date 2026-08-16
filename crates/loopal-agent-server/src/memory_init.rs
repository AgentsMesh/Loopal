use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;

use loopal_config::MemoryConfig;
use loopal_kernel::Kernel;
use loopal_memory::{
    MemorySubsystem, PROJECT_MEMORY_DIR, PROJECT_MEMORY_EVENTS_DIR, ensure_gitignore,
};
use tokio::sync::{Mutex, OnceCell};

type SubsystemCell = OnceCell<Arc<MemorySubsystem>>;
type Registry = Mutex<HashMap<String, Arc<SubsystemCell>>>;

static MEMORY_SUBSYSTEMS: OnceLock<Registry> = OnceLock::new();

pub async fn init_project_memory(
    kernel: &mut Kernel,
    cwd: &Path,
    session_id: &str,
    memory_config: &MemoryConfig,
) {
    let session_dir = match loopal_config::session_dir(session_id) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, session_id, "session_dir resolution failed; memory_recall disabled");
            return;
        }
    };
    init_project_memory_at_session_dir(kernel, cwd, session_id, memory_config, session_dir).await;
}

async fn init_project_memory_at_session_dir(
    kernel: &mut Kernel,
    cwd: &Path,
    session_id: &str,
    memory_config: &MemoryConfig,
    session_dir: PathBuf,
) {
    let memory_dir = cwd.join(PROJECT_MEMORY_DIR);
    if !memory_dir.exists() {
        tracing::debug!(
            path = %memory_dir.display(),
            "no project memory directory; skipping memory_recall init"
        );
        return;
    }

    let events_dir = cwd.join(PROJECT_MEMORY_EVENTS_DIR);

    if let Err(e) = ensure_gitignore(cwd) {
        tracing::warn!(error = %e, "memory: ensure_gitignore failed; events may end up in git");
    }

    let registry = MEMORY_SUBSYSTEMS.get_or_init(|| Mutex::new(HashMap::new()));
    let cell = {
        let mut guard = registry.lock().await;
        guard
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    };

    let gc_compress = memory_config.gc_compress_after_days;
    let gc_archive = memory_config.gc_archive_after_days;
    let init_result = cell
        .get_or_try_init(|| async {
            MemorySubsystem::bootstrap(
                memory_dir.clone(),
                session_dir.clone(),
                events_dir.clone(),
                session_id.to_string(),
                gc_compress,
                gc_archive,
            )
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "memory subsystem bootstrap failed; recall tool disabled");
            })
        })
        .await;

    let subsystem = match init_result {
        Ok(s) => s.clone(),
        Err(()) => return,
    };

    loopal_agent::tools::register_memory_recall(kernel, subsystem.graph());
    tracing::debug!(
        memory_dir = %memory_dir.display(),
        session_id,
        "memory_recall tool registered"
    );
}

#[cfg(test)]
mod tests {
    use super::init_project_memory_at_session_dir;

    #[tokio::test]
    async fn gitignore_and_bootstrap_failures_disable_memory_without_aborting_setup() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(loopal_memory::PROJECT_MEMORY_DIR)).unwrap();
        // A directory at `.gitignore` makes ensure_gitignore fail while the
        // memory bootstrap itself is forced to fail at create_dir_all below.
        std::fs::create_dir(temp.path().join(".gitignore")).unwrap();
        let session_path = temp.path().join("session-target");
        std::fs::write(&session_path, b"not a directory").unwrap();
        let mut kernel = loopal_kernel::Kernel::new(loopal_config::Settings::default()).unwrap();

        init_project_memory_at_session_dir(
            &mut kernel,
            temp.path(),
            "memory-init-test",
            &loopal_config::MemoryConfig::default(),
            session_path,
        )
        .await;

        assert!(kernel.get_tool("memory_recall").is_none());
    }
}
