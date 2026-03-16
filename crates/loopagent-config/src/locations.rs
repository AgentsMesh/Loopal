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
