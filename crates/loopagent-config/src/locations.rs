use std::path::{Path, PathBuf};

use loopagent_types::error::ConfigError;

const GLOBAL_DIR_NAME: &str = ".loopagent";
const PROJECT_DIR_NAME: &str = ".loopagent";
const SETTINGS_FILE: &str = "settings.json";
const LOCAL_SETTINGS_FILE: &str = "settings.local.json";
const INSTRUCTIONS_FILE: &str = "LOOPAGENT.md";

/// Returns the global config directory: ~/.loopagent/
pub fn global_config_dir() -> Result<PathBuf, ConfigError> {
    dirs::home_dir()
        .map(|h| h.join(GLOBAL_DIR_NAME))
        .ok_or_else(|| ConfigError::Parse("could not determine home directory".to_string()))
}

/// Returns the path to the global settings file: ~/.loopagent/settings.json
pub fn global_settings_path() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join(SETTINGS_FILE))
}

/// Returns the project config directory: <cwd>/.loopagent/
pub fn project_config_dir(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_DIR_NAME)
}

/// Returns the path to the project settings file: <cwd>/.loopagent/settings.json
pub fn project_settings_path(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join(SETTINGS_FILE)
}

/// Returns the path to the project local settings file: <cwd>/.loopagent/settings.local.json
pub fn project_local_settings_path(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join(LOCAL_SETTINGS_FILE)
}

/// Returns the path to the global instructions file: ~/.loopagent/LOOPAGENT.md
pub fn global_instructions_path() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join(INSTRUCTIONS_FILE))
}

/// Returns the path to the project instructions file: <cwd>/LOOPAGENT.md
pub fn project_instructions_path(cwd: &Path) -> PathBuf {
    cwd.join(INSTRUCTIONS_FILE)
}

/// Returns the global skills directory: ~/.loopagent/skills/
pub fn global_skills_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("skills"))
}

/// Returns the project skills directory: <cwd>/.loopagent/skills/
pub fn project_skills_dir(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join("skills")
}

// === Volatile directories (under temp_dir, infallible) ===

/// Returns the volatile data root: {temp_dir}/loopagent/
pub fn volatile_dir() -> PathBuf {
    std::env::temp_dir().join("loopagent")
}

/// Returns the log directory: {temp_dir}/loopagent/logs/
pub fn logs_dir() -> PathBuf {
    volatile_dir().join("logs")
}

/// Returns the temp file directory: {temp_dir}/loopagent/tmp/
pub fn tmp_dir() -> PathBuf {
    volatile_dir().join("tmp")
}

// === Persistent data directories ===

/// Returns the sessions root: ~/.loopagent/sessions/
pub fn sessions_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("sessions"))
}

/// Returns a single session directory: ~/.loopagent/sessions/{id}/
pub fn session_dir(id: &str) -> Result<PathBuf, ConfigError> {
    Ok(sessions_dir()?.join(id))
}

/// Returns the tasks directory for a session: ~/.loopagent/sessions/{id}/tasks/
pub fn session_tasks_dir(id: &str) -> Result<PathBuf, ConfigError> {
    Ok(session_dir(id)?.join("tasks"))
}

/// Returns the global agents directory: ~/.loopagent/agents/
pub fn global_agents_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("agents"))
}

/// Returns the project agents directory: <cwd>/.loopagent/agents/
pub fn project_agents_dir(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join("agents")
}
