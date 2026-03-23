use loopal_config::NetworkPolicy;
use loopal_sandbox::network::{check_domain, extract_domain};

#[test]
fn empty_policy_allows_all() {
    let policy = NetworkPolicy::default();
    assert!(check_domain(&policy, "example.com").is_ok());
    assert!(check_domain(&policy, "evil.org").is_ok());
}

#[test]
fn allowlist_restricts_to_listed() {
    let policy = NetworkPolicy {
        allowed_domains: vec!["github.com".to_string()],
        denied_domains: vec![],
    };
    assert!(check_domain(&policy, "github.com").is_ok());
    assert!(check_domain(&policy, "api.github.com").is_ok());
    assert!(check_domain(&policy, "evil.com").is_err());
}

#[test]
fn denylist_blocks_listed() {
    let policy = NetworkPolicy {
        allowed_domains: vec![],
        denied_domains: vec!["evil.com".to_string()],
    };
    assert!(check_domain(&policy, "evil.com").is_err());
    assert!(check_domain(&policy, "sub.evil.com").is_err());
    assert!(check_domain(&policy, "good.com").is_ok());
}

#[test]
fn case_insensitive_matching() {
    let policy = NetworkPolicy {
        allowed_domains: vec!["GitHub.COM".to_string()],
        denied_domains: vec![],
    };
    assert!(check_domain(&policy, "github.com").is_ok());
    assert!(check_domain(&policy, "GITHUB.COM").is_ok());
}

#[test]
fn extract_domain_basic() {
    assert_eq!(
        extract_domain("https://github.com/user/repo"),
        Some("github.com".to_string())
    );
    assert_eq!(
        extract_domain("http://example.com:8080/path"),
        Some("example.com".to_string())
    );
}

#[test]
fn extract_domain_no_scheme() {
    assert_eq!(
        extract_domain("github.com/path"),
        Some("github.com".to_string())
    );
}

#[test]
fn extract_domain_empty() {
    assert_eq!(extract_domain("https://"), None);
    assert_eq!(extract_domain(""), None);
}

#[test]
fn both_allow_and_deny_applied() {
    let policy = NetworkPolicy {
        allowed_domains: vec!["example.com".to_string()],
        denied_domains: vec!["bad.example.com".to_string()],
    };
    assert!(check_domain(&policy, "example.com").is_ok());
    // Sub-domain of allowed is allowed
    assert!(check_domain(&policy, "api.example.com").is_ok());
    // But explicitly denied sub-domain is blocked
    assert!(check_domain(&policy, "bad.example.com").is_err());
}
