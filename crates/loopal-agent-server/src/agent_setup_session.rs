use loopal_runtime::SessionManager;
use loopal_storage::Session;
use loopal_turn::Turn;

use crate::params::StartParams;

pub(crate) fn open(
    cwd: &std::path::Path,
    model: &str,
    session_id: &str,
    session_dir_override: Option<&std::path::Path>,
    start: &StartParams,
) -> anyhow::Result<(SessionManager, Session, Vec<Turn>)> {
    let manager = session_manager(session_dir_override)?;
    let (session, resume_turns) = if let Some(resume_id) = start.resume.as_deref() {
        manager.resume_session(resume_id)?
    } else {
        (
            manager.create_session_with_id(cwd, model, session_id)?,
            Vec::new(),
        )
    };
    let initial_turns = crate::agent_initial_turns::initial_turns_for_start(
        start,
        &manager,
        &session.id,
        resume_turns,
    )?;
    Ok((manager, session, initial_turns))
}

fn session_manager(
    session_dir_override: Option<&std::path::Path>,
) -> anyhow::Result<SessionManager> {
    if let Some(dir) = session_dir_override {
        Ok(SessionManager::with_base_dir(dir.to_path_buf()))
    } else {
        Ok(SessionManager::new()?)
    }
}

#[cfg(test)]
mod tests {
    use super::session_manager;

    #[test]
    fn session_manager_supports_override_and_default_locations() {
        let temp = tempfile::tempdir().unwrap();
        session_manager(Some(temp.path())).unwrap();
        session_manager(None).unwrap();
    }
}
