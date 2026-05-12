use loopal_config::HarnessConfig;
use loopal_config::loader::apply_env_overrides;

#[test]
fn classifier_timeout_default_is_180_seconds() {
    let h = HarnessConfig::default();
    assert_eq!(h.classifier_timeout_secs, 180);
}

#[test]
fn env_override_round_trip() {
    // Single combined test to avoid env-var races between parallel tests.
    struct EnvGuard(&'static str, Option<String>);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.1 {
                Some(v) => unsafe { std::env::set_var(self.0, v) },
                None => unsafe { std::env::remove_var(self.0) },
            }
        }
    }
    let prev = std::env::var("LOOPAL_CLASSIFIER_TIMEOUT_SECS").ok();
    let _g = EnvGuard("LOOPAL_CLASSIFIER_TIMEOUT_SECS", prev);

    // Valid integer is written into harness.
    unsafe { std::env::set_var("LOOPAL_CLASSIFIER_TIMEOUT_SECS", "300") };
    let mut v = serde_json::json!({});
    apply_env_overrides(&mut v);
    assert_eq!(v["harness"]["classifier_timeout_secs"], 300);

    // Invalid (non-numeric) is silently skipped; the key from the prior
    // call still exists on the same JSON value, so use a fresh object.
    unsafe { std::env::set_var("LOOPAL_CLASSIFIER_TIMEOUT_SECS", "abc") };
    let mut v2 = serde_json::json!({});
    apply_env_overrides(&mut v2);
    assert!(
        v2.get("harness").is_none() || v2["harness"].get("classifier_timeout_secs").is_none(),
        "invalid value must not be written; got {v2}"
    );

    // Unset entirely → no harness override
    unsafe { std::env::remove_var("LOOPAL_CLASSIFIER_TIMEOUT_SECS") };
    let mut v3 = serde_json::json!({});
    apply_env_overrides(&mut v3);
    assert!(v3.get("harness").is_none());
}
