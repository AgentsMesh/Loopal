use loopal_auto_mode::CircuitBreaker;

#[test]
fn starts_not_degraded() {
    let cb = CircuitBreaker::new();
    assert!(!cb.is_degraded());
}

#[test]
fn approval_does_not_degrade() {
    let cb = CircuitBreaker::new();
    for _ in 0..100 {
        cb.record_approval("Bash");
    }
    assert!(!cb.is_degraded());
}

#[test]
fn consecutive_denials_degrade() {
    let cb = CircuitBreaker::new();
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    assert!(!cb.is_degraded());
    cb.record_denial("Bash"); // 3rd consecutive → degrade
    assert!(cb.is_degraded());
}

#[test]
fn approval_resets_consecutive_count() {
    let cb = CircuitBreaker::new();
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    cb.record_approval("Bash"); // reset
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    assert!(!cb.is_degraded()); // only 2 consecutive, not 3
}

#[test]
fn total_denials_degrade() {
    let cb = CircuitBreaker::new();
    // 20 different tools, 1 denial each → total threshold
    for i in 0..19 {
        cb.record_denial(&format!("Tool{i}"));
        assert!(!cb.is_degraded());
    }
    cb.record_denial("Tool19"); // 20th total → degrade
    assert!(cb.is_degraded());
}

#[test]
fn error_counts_as_denial() {
    let cb = CircuitBreaker::new();
    cb.record_error("Bash");
    cb.record_error("Bash");
    cb.record_error("Bash"); // 3rd → degrade
    assert!(cb.is_degraded());
}

#[test]
fn reset_degradation_clears_state() {
    let cb = CircuitBreaker::new();
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    assert!(cb.is_degraded());

    cb.reset_degradation();
    assert!(!cb.is_degraded());

    // Consecutive counters also reset — need 3 more to degrade again
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    assert!(!cb.is_degraded());
}

#[test]
fn reset_after_total_denials_fully_recovers() {
    let cb = CircuitBreaker::new();
    // 20 different tools → total threshold → degrade
    for i in 0..20 {
        cb.record_denial(&format!("Tool{i}"));
    }
    assert!(cb.is_degraded());

    cb.reset_degradation();
    assert!(!cb.is_degraded());

    // After reset, total_denials must also be zero.
    // A single denial should NOT re-trigger degradation.
    cb.record_denial("NewTool");
    assert!(!cb.is_degraded());
}

#[test]
fn different_tools_have_independent_consecutive_counts() {
    let cb = CircuitBreaker::new();
    cb.record_denial("Bash");
    cb.record_denial("Bash");
    cb.record_denial("Write"); // different tool, Bash still at 2
    assert!(!cb.is_degraded());
    cb.record_denial("Bash"); // Bash 3rd → degrade
    assert!(cb.is_degraded());
}
