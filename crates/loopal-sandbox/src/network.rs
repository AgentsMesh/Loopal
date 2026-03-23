use loopal_config::NetworkPolicy;

/// Check whether a domain is allowed under the given network policy.
///
/// Returns `Ok(())` if the domain is allowed, or `Err(reason)` if blocked.
pub fn check_domain(policy: &NetworkPolicy, domain: &str) -> Result<(), String> {
    let domain_lower = domain.to_lowercase();

    // If an allowlist is configured, only those domains pass
    if !policy.allowed_domains.is_empty() {
        let allowed = policy.allowed_domains.iter().any(|d| {
            let d_lower = d.to_lowercase();
            domain_lower == d_lower || domain_lower.ends_with(&format!(".{d_lower}"))
        });
        if !allowed {
            return Err(format!("domain '{domain}' not in allowlist"));
        }
    }

    // Check deny list
    let denied = policy.denied_domains.iter().any(|d| {
        let d_lower = d.to_lowercase();
        domain_lower == d_lower || domain_lower.ends_with(&format!(".{d_lower}"))
    });
    if denied {
        return Err(format!("domain '{domain}' is in deny list"));
    }

    Ok(())
}

/// Extract the domain from a URL string (best-effort).
pub fn extract_domain(url: &str) -> Option<String> {
    // Strip scheme
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    // Take everything before the first '/' or ':'
    let domain = without_scheme.split('/').next()?.split(':').next()?;

    if domain.is_empty() {
        return None;
    }
    Some(domain.to_string())
}
