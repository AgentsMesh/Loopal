use loopal_config::LocalSettingsFieldPatch;
use loopal_ipc::protocol::methods::{DesktopMcpCwdIsolation, DesktopMcpServerInput};
use serde_json::json;

use super::{desktop_mcp_secret_patches, desktop_mcp_url};

pub(super) fn patches(
    server: DesktopMcpServerInput,
) -> Result<Vec<LocalSettingsFieldPatch>, String> {
    match server {
        DesktopMcpServerInput::Stdio {
            name,
            command,
            args,
            enabled,
            timeout_ms,
            sharing,
            cwd_isolation,
            secret_patches: secrets,
        } => {
            validate_name("name", &name, 64)?;
            validate_text("command", &command, 1024, false)?;
            validate_args(&args)?;
            validate_timeout(timeout_ms)?;
            let prefix = format!("mcp_servers.{name}");
            let mut out = base(&prefix, "stdio", enabled, timeout_ms, sharing);
            out.extend([
                set(&prefix, "command", json!(command)),
                set(&prefix, "args", json!(args)),
                remove(&prefix, "url"),
                remove(&prefix, "headers"),
            ]);
            match cwd_isolation {
                Some(value) => {
                    validate_isolation(&value)?;
                    out.push(set(
                        &prefix,
                        "cwd_isolation",
                        json!({
                            "arg": value.arg, "cache_subdir": value.cache_subdir,
                        }),
                    ));
                }
                None => out.push(remove(&prefix, "cwd_isolation")),
            }
            out.extend(desktop_mcp_secret_patches::build(&prefix, secrets, "env")?);
            Ok(out)
        }
        DesktopMcpServerInput::StreamableHttp {
            name,
            url,
            enabled,
            timeout_ms,
            sharing,
            secret_patches: secrets,
        } => {
            validate_name("name", &name, 64)?;
            desktop_mcp_url::validate(&url)?;
            validate_timeout(timeout_ms)?;
            let prefix = format!("mcp_servers.{name}");
            let mut out = base(&prefix, "streamable-http", enabled, timeout_ms, sharing);
            out.extend([
                set(&prefix, "url", json!(url)),
                remove(&prefix, "command"),
                remove(&prefix, "args"),
                remove(&prefix, "env"),
                remove(&prefix, "cwd_isolation"),
            ]);
            out.extend(desktop_mcp_secret_patches::build(
                &prefix, secrets, "headers",
            )?);
            Ok(out)
        }
    }
}

pub(super) fn validate_server_name(name: &str) -> Result<(), String> {
    validate_name("name", name, 64)
}

fn base(
    prefix: &str,
    kind: &str,
    enabled: bool,
    timeout_ms: u64,
    sharing: impl serde::Serialize,
) -> Vec<LocalSettingsFieldPatch> {
    vec![
        set(prefix, "type", json!(kind)),
        set(prefix, "enabled", json!(enabled)),
        set(prefix, "timeout_ms", json!(timeout_ms)),
        set(prefix, "sharing", json!(sharing)),
    ]
}

fn validate_name(field: &str, value: &str, max: usize) -> Result<(), String> {
    let first_ok = value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric);
    let rest_ok = value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if !first_ok || !rest_ok || value.len() > max {
        return Err(format!(
            "{field} must be an alphanumeric, underscore, or hyphen identifier"
        ));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max: usize, allow_empty: bool) -> Result<(), String> {
    if (!allow_empty && value.trim().is_empty())
        || value.len() > max
        || value.chars().any(char::is_control)
    {
        return Err(format!("{field} is invalid"));
    }
    Ok(())
}

fn validate_args(args: &[String]) -> Result<(), String> {
    if args.len() > 128 {
        return Err("args must contain at most 128 entries".into());
    }
    args.iter()
        .try_for_each(|arg| validate_text("argument", arg, 4096, true))
}

fn validate_timeout(value: u64) -> Result<(), String> {
    (100..=600_000)
        .contains(&value)
        .then_some(())
        .ok_or_else(|| "timeoutMs must be between 100 and 600000".into())
}

fn validate_isolation(value: &DesktopMcpCwdIsolation) -> Result<(), String> {
    if !value.arg.starts_with('-') {
        return Err("cwd isolation arg must start with '-'".into());
    }
    validate_text("cwd isolation arg", &value.arg, 128, false)?;
    if let Some(subdir) = &value.cache_subdir {
        validate_name("cwd isolation cacheSubdir", subdir, 128)?;
    }
    Ok(())
}

fn set(prefix: &str, field: &str, value: serde_json::Value) -> LocalSettingsFieldPatch {
    LocalSettingsFieldPatch::Set(format!("{prefix}.{field}"), value)
}

fn remove(prefix: &str, field: &str) -> LocalSettingsFieldPatch {
    LocalSettingsFieldPatch::Remove(format!("{prefix}.{field}"))
}
