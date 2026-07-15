pub(crate) fn join_v1(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');
    if base.ends_with("/v1") {
        let endpoint = endpoint.strip_prefix("v1/").unwrap_or(endpoint);
        format!("{base}/{endpoint}")
    } else {
        format!("{base}/{endpoint}")
    }
}

#[cfg(test)]
mod tests {
    use super::join_v1;

    #[test]
    fn joins_origin_and_versioned_base_urls() {
        assert_eq!(
            join_v1("https://api.example", "/v1/responses"),
            "https://api.example/v1/responses"
        );
        assert_eq!(
            join_v1("https://api.example/v1/", "/v1/responses"),
            "https://api.example/v1/responses"
        );
    }
}
