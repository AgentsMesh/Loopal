//! Sensitive file patterns and environment variable names that should be
//! protected by the sandbox. These constants are used by `path_checker`,
//! `env_sanitizer`, and `scanner`.

/// File globs that should never be written by the agent.
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
];

/// Environment variable name patterns (case-insensitive substring match)
/// that are considered sensitive and should be stripped.
pub const SENSITIVE_ENV_PATTERNS: &[&str] = &[
    // Cloud provider secrets
    "AWS_SECRET",
    "AWS_SESSION_TOKEN",
    "AWS_ACCESS_KEY",
    "AZURE_CLIENT_SECRET",
    "AZURE_TENANT",
    "GCP_SERVICE_ACCOUNT",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_API_KEY",
    "GCLOUD_",
    // Generic secret / token / key patterns
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "PASSWD",
    "PRIVATE_KEY",
    "API_KEY",
    "APIKEY",
    "AUTH_KEY",
    "ACCESS_KEY",
    "ENCRYPTION_KEY",
    "SIGNING_KEY",
    "JWT_SECRET",
    "SESSION_SECRET",
    "COOKIE_SECRET",
    // Database credentials
    "DATABASE_URL",
    "DB_PASSWORD",
    "DB_PASS",
    "MONGO_URI",
    "REDIS_URL",
    "REDIS_PASSWORD",
    "MYSQL_PASSWORD",
    "POSTGRES_PASSWORD",
    "PGPASSWORD",
    // CI / SCM tokens
    "GITHUB_TOKEN",
    "GITLAB_TOKEN",
    "BITBUCKET_TOKEN",
    "NPM_TOKEN",
    "PYPI_TOKEN",
    "DOCKER_PASSWORD",
    "DOCKER_AUTH",
    "REGISTRY_PASSWORD",
    "CI_JOB_TOKEN",
    "CIRCLE_TOKEN",
    "TRAVIS_TOKEN",
    // Messaging / notification
    "SLACK_TOKEN",
    "SLACK_WEBHOOK",
    "DISCORD_TOKEN",
    "TELEGRAM_TOKEN",
    "TWILIO_AUTH",
    "SENDGRID_API_KEY",
    "MAILGUN_API_KEY",
    // Payment / SaaS
    "STRIPE_SECRET",
    "STRIPE_KEY",
    "PAYPAL_SECRET",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    // SSH / signing
    "SSH_PRIVATE",
    "GPG_PASSPHRASE",
    "SIGNING_SECRET",
    // Encryption
    "MASTER_KEY",
    "SALT",
    "HMAC",
    "CIPHER",
];

/// Environment variable names that are always safe to keep.
pub const SAFE_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "SHELL",
    "TERM",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "TMPDIR",
    "XDG_RUNTIME_DIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "EDITOR",
    "VISUAL",
    "PAGER",
    "COLORTERM",
    "TERM_PROGRAM",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "GOPATH",
    "GOROOT",
    "NODE_PATH",
    "NVM_DIR",
    "PYENV_ROOT",
    "VIRTUAL_ENV",
    "CONDA_PREFIX",
    "JAVA_HOME",
    "ANDROID_HOME",
    "PWD",
    "OLDPWD",
    "SHLVL",
    "HOSTNAME",
    "LOGNAME",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    // Loopal's own env vars are always kept
    "LOOPAL_MODEL",
    "LOOPAL_MAX_TURNS",
    "LOOPAL_PERMISSION_MODE",
    "LOOPAL_LOG",
    "LOOPAL_SANDBOX",
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

/// Shell operators that may indicate command injection.
pub const SHELL_INJECTION_PATTERNS: &[&str] = &[
    "$(", "`",   // Command substitution
    "&&", "||",  // Chaining (allowed in normal use but flagged in sandbox)
    "|",         // Piping
    ";",         // Command separator
    ">",         // Redirect
    "<",         // Input redirect
];
