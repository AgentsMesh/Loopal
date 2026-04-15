//! Sensitive file patterns and dangerous command patterns for the sandbox.
//!
//! Used by `path_checker` (file globs), `command_checker` (command patterns),
//! and `scanner`. Environment variable patterns live in `env_patterns`.

/// File globs that should never be written by the agent without approval.
pub const SENSITIVE_FILE_GLOBS: &[&str] = &[
    "**/.env",
    "**/.env.*",
    "**/.env.local",
    "**/.env.production",
    "**/credentials.json",
    "**/service-account*.json",
    "**/*.pem",
    "**/*.key",
    "**/*.p12",
    "**/*.pfx",
    "**/*.keystore",
    "**/*.jks",
    "**/*secret*",
    "**/.ssh/*",
    "**/.gnupg/*",
    "**/.aws/credentials",
    "**/.azure/accessTokens.json",
    "**/.config/gcloud/**",
    "**/.npmrc",
    "**/.pypirc",
    "**/.docker/config.json",
    "**/.kube/config",
    "**/id_rsa*",
    "**/id_ed25519*",
    "**/id_ecdsa*",
    "**/.git/config",
    "**/.netrc",
    "**/.htpasswd",
    "**/*.sqlite",
    "**/*.db",
    // Shell configs (code injection vector)
    "**/.bashrc",
    "**/.bash_profile",
    "**/.bash_login",
    "**/.zshrc",
    "**/.zprofile",
    "**/.zshenv",
    "**/.profile",
    "**/.login",
    // SSH authorized keys
    "**/authorized_keys",
    // macOS persistence vectors
    "**/LaunchAgents/**",
    "**/LaunchDaemons/**",
];

/// Dangerous command patterns that should be blocked entirely.
pub const DANGEROUS_COMMAND_PATTERNS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "mkfs.",
    "dd if=",
    ":(){ :|:& };:",
    "chmod -R 777 /",
    "chown -R",
    "> /dev/sda",
    "shutdown",
    "reboot",
    "init 0",
    "init 6",
    "halt",
    "poweroff",
];
