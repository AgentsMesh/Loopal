pub fn apply_env_overrides(value: &mut serde_json::Value) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }

    if let Ok(model) = std::env::var("LOOPAL_MODEL") {
        value["model"] = serde_json::Value::String(model);
    }

    if let Ok(mode) = std::env::var("LOOPAL_PERMISSION_MODE") {
        value["permission_mode"] = serde_json::Value::String(mode);
    }

    if let Ok(mode) = std::env::var("LOOPAL_DECISION_MODE") {
        value["decision_mode"] = serde_json::Value::String(mode);
    }

    if let Ok(sandbox) = std::env::var("LOOPAL_SANDBOX") {
        value["sandbox"]["policy"] = serde_json::Value::String(sandbox);
    }

    if let Ok(t) = std::env::var("LOOPAL_CLASSIFIER_TIMEOUT_SECS")
        && let Ok(parsed) = t.parse::<u64>()
    {
        if !value["harness"].is_object() {
            value["harness"] = serde_json::json!({});
        }
        value["harness"]["classifier_timeout_secs"] =
            serde_json::Value::Number(serde_json::Number::from(parsed));
    }
}
