use std::collections::{HashMap, HashSet};

use loopal_config::LocalSettingsFieldPatch;
use loopal_ipc::protocol::methods::{
    DesktopMcpSecretOperation, DesktopMcpSecretPatch, DesktopMcpSecretTarget,
};
use serde_json::json;

pub(super) fn build(
    prefix: &str,
    patches: Vec<DesktopMcpSecretPatch>,
    expected: &str,
) -> Result<Vec<LocalSettingsFieldPatch>, String> {
    if patches.len() > 256 {
        return Err("secretPatches must contain at most 256 entries".into());
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(patches.len());
    for patch in patches {
        let target = match patch.target {
            DesktopMcpSecretTarget::Env => "env",
            DesktopMcpSecretTarget::Header => "headers",
        };
        if target != expected {
            return Err(format!("{expected} secret patches must target {expected}"));
        }
        validate_name(target, &patch.name)?;
        let identity = if target == "headers" {
            patch.name.to_ascii_lowercase()
        } else {
            patch.name.clone()
        };
        if !seen.insert(identity) {
            return Err(format!("duplicate secret patch for {}", patch.name));
        }
        let path = format!("{prefix}.{target}.{}", patch.name);
        match (patch.operation, patch.value) {
            (DesktopMcpSecretOperation::Set, Some(value)) => {
                validate_value(target, &value)?;
                out.push(LocalSettingsFieldPatch::Set(path, json!(value)));
            }
            (DesktopMcpSecretOperation::Remove, None) => {
                out.push(LocalSettingsFieldPatch::Remove(path));
            }
            (DesktopMcpSecretOperation::Set, None) => {
                return Err("set secret patch requires a value".into());
            }
            (DesktopMcpSecretOperation::Remove, Some(_)) => {
                return Err("remove secret patch must not include a value".into());
            }
        }
    }
    Ok(out)
}

pub(super) fn validate_existing_name(target: &str, name: &str) -> Result<(), String> {
    validate_name(target, name)
        .map_err(|_| format!("existing {target} secret name cannot be safely edited by Desktop"))
}

pub(super) fn validate_header_uniqueness(headers: &HashMap<String, String>) -> Result<(), String> {
    let mut seen = HashSet::new();
    if headers
        .keys()
        .any(|name| !seen.insert(name.to_ascii_lowercase()))
    {
        return Err("existing headers contain ASCII case-insensitive duplicates".into());
    }
    Ok(())
}

pub(super) fn validate_header_edit(
    headers: &HashMap<String, String>,
    patch_names: &[String],
) -> Result<(), String> {
    validate_header_uniqueness(headers)?;
    for patch in patch_names {
        if headers
            .keys()
            .any(|existing| existing != patch && existing.eq_ignore_ascii_case(patch))
        {
            return Err("header patch casing must match the configured header name".into());
        }
    }
    Ok(())
}

fn validate_name(target: &str, name: &str) -> Result<(), String> {
    if target == "env" {
        let first = name
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_');
        let rest = name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if !first || !rest || name.len() > 128 {
            return Err("environment key is invalid".into());
        }
    } else {
        let valid = name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-^_`|~".contains(&b));
        if name.is_empty() || !valid || name.len() > 128 {
            return Err("header name is invalid".into());
        }
    }
    Ok(())
}

fn validate_value(target: &str, value: &str) -> Result<(), String> {
    let invalid = value.is_empty()
        || value.len() > 8192
        || value.contains('\0')
        || (target == "headers" && value.parse::<reqwest::header::HeaderValue>().is_err());
    (!invalid)
        .then_some(())
        .ok_or_else(|| "secret value is invalid".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(name: &str) -> DesktopMcpSecretPatch {
        DesktopMcpSecretPatch {
            target: DesktopMcpSecretTarget::Header,
            name: name.into(),
            operation: DesktopMcpSecretOperation::Set,
            value: Some("value".into()),
        }
    }

    #[test]
    fn headers_are_case_insensitive_and_patch_count_is_bounded() {
        let error = build(
            "mcp_servers.test",
            vec![set("Authorization"), set("authorization")],
            "headers",
        )
        .unwrap_err();
        assert!(error.contains("duplicate secret patch"));
        let many = (0..257).map(|index| set(&format!("X-{index}"))).collect();
        assert!(
            build("mcp_servers.test", many, "headers")
                .unwrap_err()
                .contains("256")
        );
        let mut control = set("X-Control");
        control.value = Some("bad\u{7f}".into());
        assert_eq!(
            build("mcp_servers.test", vec![control], "headers").unwrap_err(),
            "secret value is invalid"
        );
    }

    #[test]
    fn inherited_header_duplicates_are_rejected() {
        let headers = HashMap::from([
            ("Authorization".into(), "first".into()),
            ("authorization".into(), "second".into()),
        ]);
        assert!(validate_header_uniqueness(&headers).is_err());
        let unique = HashMap::from([("Authorization".into(), "first".into())]);
        assert!(validate_header_edit(&unique, &["authorization".into()]).is_err());
    }
}
