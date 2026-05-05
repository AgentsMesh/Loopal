use std::path::{Path, PathBuf};

use loopal_error::ConfigError;

const GLOBAL_DIR_NAME: &str = ".loopal";
const PROJECT_DIR_NAME: &str = ".loopal";
const SETTINGS_FILE: &str = "settings.json";
const LOCAL_SETTINGS_FILE: &str = "settings.local.json";
const INSTRUCTIONS_FILE: &str = "LOOPAL.md";
const LOCAL_INSTRUCTIONS_FILE: &str = "LOOPAL.local.md";

/// Returns the global config directory: ~/.loopal/
///
/// Returns `Err` when the home directory cannot be determined (e.g. `HOME` is
/// unset or empty, which happens inside Bazel test sandboxes).
pub fn global_config_dir() -> Result<PathBuf, ConfigError> {
    dirs::home_dir()
        .filter(|h| !h.as_os_str().is_empty())
        .map(|h| h.join(GLOBAL_DIR_NAME))
        .ok_or_else(|| ConfigError::Parse("could not determine home directory".to_string()))
}

/// Returns the path to the global settings file: ~/.loopal/settings.json
pub fn global_settings_path() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join(SETTINGS_FILE))
}

/// Returns the project config directory: <cwd>/.loopal/
pub fn project_config_dir(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_DIR_NAME)
}

/// Returns the path to the project settings file: <cwd>/.loopal/settings.json
pub fn project_settings_path(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join(SETTINGS_FILE)
}

/// Returns the path to the project local settings file: <cwd>/.loopal/settings.local.json
pub fn project_local_settings_path(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join(LOCAL_SETTINGS_FILE)
}

/// Returns the path to the global instructions file: ~/.loopal/LOOPAL.md
pub fn global_instructions_path() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join(INSTRUCTIONS_FILE))
}

/// Returns the path to the global local instructions: ~/.loopal/LOOPAL.local.md
pub fn global_local_instructions_path() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join(LOCAL_INSTRUCTIONS_FILE))
}

/// Returns the path to the project instructions file: <cwd>/LOOPAL.md
pub fn project_instructions_path(cwd: &Path) -> PathBuf {
    cwd.join(INSTRUCTIONS_FILE)
}

/// Returns the path to the project local instructions: <cwd>/.loopal/LOOPAL.local.md
pub fn project_local_instructions_path(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join(LOCAL_INSTRUCTIONS_FILE)
}

/// Returns the global skills directory: ~/.loopal/skills/
pub fn global_skills_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("skills"))
}

/// Returns the project skills directory: <cwd>/.loopal/skills/
pub fn project_skills_dir(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join("skills")
}

/// Returns the global plugins directory: ~/.loopal/plugins/
pub fn global_plugins_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("plugins"))
}

// === Volatile directories (under temp_dir, infallible) ===

/// Returns the volatile data root: {temp_dir}/loopal/
pub fn volatile_dir() -> PathBuf {
    std::env::temp_dir().join("loopal")
}

/// Returns the temp file directory: {temp_dir}/loopal/tmp/
pub fn tmp_dir() -> PathBuf {
    volatile_dir().join("tmp")
}

// === Persistent data directories ===

/// Returns the log directory: ~/.loopal/logs/
/// Fallback to {temp_dir}/loopal/logs/ if home directory is unavailable.
pub fn logs_dir() -> PathBuf {
    global_config_dir()
        .map(|d| d.join("logs"))
        .unwrap_or_else(|_| volatile_dir().join("logs"))
}

/// Returns the sessions root: ~/.loopal/sessions/
pub fn sessions_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("sessions"))
}

/// Returns a single session directory: ~/.loopal/sessions/{id}/
pub fn session_dir(id: &str) -> Result<PathBuf, ConfigError> {
    Ok(sessions_dir()?.join(id))
}

/// Returns the runtime data directory: ~/.loopal/run/
/// Used for hub discovery records (`<pid>.json`) and per-pid Unix
/// sockets used for token handoff. Falls back to volatile temp dir
/// when the home directory is unavailable.
pub fn run_dir() -> PathBuf {
    global_config_dir()
        .map(|d| d.join("run"))
        .unwrap_or_else(|_| volatile_dir().join("run"))
}

/// Returns the tasks directory for a session: ~/.loopal/sessions/{id}/tasks/
pub fn session_tasks_dir(id: &str) -> Result<PathBuf, ConfigError> {
    Ok(session_dir(id)?.join("tasks"))
}

/// Returns the global agents directory: ~/.loopal/agents/
pub fn global_agents_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("agents"))
}

/// Returns the project agents directory: <cwd>/.loopal/agents/
pub fn project_agents_dir(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join("agents")
}

/// Returns the global prompts override directory: ~/.loopal/prompts/
pub fn global_prompts_dir() -> Result<PathBuf, ConfigError> {
    Ok(global_config_dir()?.join("prompts"))
}

/// Returns the project prompts override directory: <cwd>/.loopal/prompts/
pub fn project_prompts_dir(cwd: &Path) -> PathBuf {
    project_config_dir(cwd).join("prompts")
}
