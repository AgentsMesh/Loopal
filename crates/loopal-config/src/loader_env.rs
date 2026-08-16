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
        set_u64(value, &["harness", "classifier_timeout_secs"], parsed);
    }

    apply_workflow_overrides(value);
}

fn apply_workflow_overrides(value: &mut serde_json::Value) {
    set_env_string(value, "LOOPAL_WORKFLOW_POLICY", &["workflow", "policy"]);
    set_env_string(
        value,
        "LOOPAL_WORKFLOW_PLANNER_PROFILE",
        &["workflow", "planner_profile"],
    );
    for (env, path) in [
        ("LOOPAL_WORKFLOW_MAX_NODES", "max_nodes"),
        ("LOOPAL_WORKFLOW_MAX_PARALLEL", "max_parallel"),
        ("LOOPAL_WORKFLOW_MAX_ATTEMPTS", "max_attempts"),
        ("LOOPAL_WORKFLOW_MAX_OUTPUT_BYTES", "max_output_bytes"),
    ] {
        set_env_u64(value, env, &["workflow", "limits", path]);
    }
    for (env, path) in [
        ("LOOPAL_WORKFLOW_RUN_DEADLINE_SECS", "run_deadline_secs"),
        (
            "LOOPAL_WORKFLOW_ATTEMPT_TIMEOUT_SECS",
            "attempt_timeout_secs",
        ),
        ("LOOPAL_WORKFLOW_CANCEL_GRACE_SECS", "cancel_grace_secs"),
        ("LOOPAL_WORKFLOW_RECOVERY_GRACE_SECS", "recovery_grace_secs"),
    ] {
        set_env_u64(value, env, &["workflow", "timing", path]);
    }
}

fn set_env_string(value: &mut serde_json::Value, env: &str, path: &[&str]) {
    if let Ok(raw) = std::env::var(env) {
        set_value(value, path, serde_json::Value::String(raw));
    }
}

fn set_env_u64(value: &mut serde_json::Value, env: &str, path: &[&str]) {
    if let Ok(raw) = std::env::var(env)
        && let Ok(parsed) = raw.parse::<u64>()
    {
        set_u64(value, path, parsed);
    }
}

fn set_u64(value: &mut serde_json::Value, path: &[&str], number: u64) {
    set_value(
        value,
        path,
        serde_json::Value::Number(serde_json::Number::from(number)),
    );
}

fn set_value(value: &mut serde_json::Value, path: &[&str], replacement: serde_json::Value) {
    let mut current = value;
    for segment in &path[..path.len() - 1] {
        if !current[*segment].is_object() {
            current[*segment] = serde_json::json!({});
        }
        current = &mut current[*segment];
    }
    current[path[path.len() - 1]] = replacement;
}
