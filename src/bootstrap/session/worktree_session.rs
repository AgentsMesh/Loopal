use std::path::PathBuf;

/// Holds worktree info for cleanup on exit.
pub struct SessionWorktree {
    pub info: loopal_git::WorktreeInfo,
    pub repo_root: PathBuf,
}

pub fn create_session_worktree(cwd: &std::path::Path) -> anyhow::Result<SessionWorktree> {
    let repo_root = loopal_git::repo_root(cwd)
        .ok_or_else(|| anyhow::anyhow!("--worktree requires a git repository"))?;
    let name = format!("session-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let info = loopal_git::create_worktree(&repo_root, &name)
        .map_err(|e| anyhow::anyhow!("failed to create worktree: {e}"))?;
    tracing::info!(worktree = %info.path.display(), branch = %info.branch, "session worktree created");
    Ok(SessionWorktree { info, repo_root })
}

pub fn cleanup_session_worktree(wt: &SessionWorktree) -> bool {
    let removed = loopal_git::cleanup_if_clean(&wt.repo_root, &wt.info);
    if removed {
        tracing::info!("session worktree removed (no changes)");
    } else {
        tracing::info!(
            worktree = %wt.info.path.display(),
            "worktree has changes, keeping for manual review"
        );
    }
    removed
}

pub fn print_resume_info(session_id: &str, worktree: Option<&SessionWorktree>) {
    eprintln!();
    eprintln!("To resume this session:");
    eprintln!("  loopal --resume {session_id}");
    if let Some(wt) = worktree {
        let display = super::abbreviate_home(&wt.info.path);
        eprintln!();
        eprintln!("Session worktree: {display}");
        eprintln!("  cd {display} && loopal --resume {session_id}");
    }
}

pub fn print_detach_worktree_info(worktree: Option<&SessionWorktree>) {
    if let Some(wt) = worktree {
        let display = super::abbreviate_home(&wt.info.path);
        eprintln!();
        eprintln!("Hub still owns worktree: {display}");
    }
}

pub fn print_error_worktree_info(worktree: Option<&SessionWorktree>) {
    if let Some(wt) = worktree {
        let display = super::abbreviate_home(&wt.info.path);
        eprintln!();
        eprintln!("Worktree retained at: {display}");
    }
}

pub fn resolve_resume_for_cwd(cwd: &std::path::Path) -> Option<String> {
    let sm = match loopal_runtime::SessionManager::new() {
        Ok(sm) => sm,
        Err(e) => {
            tracing::warn!("failed to create session manager for resume: {e}");
            return None;
        }
    };
    match sm.latest_session_for_cwd(cwd) {
        Ok(Some(session)) => {
            tracing::info!(session_id = %session.id, "auto-resuming latest session for cwd");
            Some(session.id)
        }
        Ok(None) => {
            tracing::info!("no previous session found for cwd, starting fresh");
            None
        }
        Err(e) => {
            tracing::warn!("failed to query sessions for resume: {e}");
            None
        }
    }
}
