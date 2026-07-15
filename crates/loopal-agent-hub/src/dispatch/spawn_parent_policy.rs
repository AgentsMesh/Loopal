use serde_json::Value;

pub(super) fn normalize_target_hub_value(value: Option<&Value>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let target = value
        .as_str()
        .ok_or_else(|| format!("'target_hub' must be a string, got: {value}"))?
        .trim();
    Ok((!target.is_empty()).then(|| target.to_string()))
}

pub(super) fn is_self_target(own_hub: Option<&str>, target: &str) -> bool {
    own_hub == Some(target)
}

pub(super) fn local_parent_policy(
    params: &Value,
    from_agent: &str,
) -> Result<(Option<String>, bool), String> {
    let notify = match params.get("notify_parent_on_completion") {
        None => true,
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "'notify_parent_on_completion' must be a boolean".to_string())?,
    };
    let parent = params["parent"]
        .as_str()
        .map(String::from)
        .or_else(|| Some(from_agent.to_string()));
    Ok((parent, notify))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{is_self_target, local_parent_policy, normalize_target_hub_value};

    #[test]
    fn recognizes_self_target() {
        assert!(is_self_target(Some("hub-a"), "hub-a"));
        assert!(!is_self_target(Some("hub-a"), "hub-b"));
        assert!(!is_self_target(None, "hub-a"));
    }

    #[test]
    fn normalizes_target_hub() {
        assert_eq!(normalize_target_hub_value(None).unwrap(), None);
        assert_eq!(
            normalize_target_hub_value(Some(&json!("   "))).unwrap(),
            None
        );
        assert_eq!(
            normalize_target_hub_value(Some(&json!(" hub-b "))).unwrap(),
            Some("hub-b".into()),
        );
        assert!(normalize_target_hub_value(Some(&json!(42))).is_err());
    }

    #[test]
    fn defaults_to_caller_and_notifies() {
        assert_eq!(
            local_parent_policy(&json!({}), "main").unwrap(),
            (Some("main".into()), true),
        );
    }

    #[test]
    fn suppression_retains_parent_topology() {
        assert_eq!(
            local_parent_policy(&json!({"notify_parent_on_completion": false}), "main",).unwrap(),
            (Some("main".into()), false),
        );
    }

    #[test]
    fn rejects_non_boolean_policy() {
        assert!(
            local_parent_policy(&json!({"notify_parent_on_completion": "false"}), "main",).is_err()
        );
    }
}
