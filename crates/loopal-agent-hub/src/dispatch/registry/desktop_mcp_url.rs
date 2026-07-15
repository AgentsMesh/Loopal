use reqwest::Url;

const REDACTED_URL: &str = "https://redacted.invalid/";

pub(super) fn validate(value: &str) -> Result<(), String> {
    let url = parse(value).ok_or_else(invalid)?;
    if has_userinfo_marker(value)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn project(value: &str) -> String {
    let Some(mut url) = parse(value) else {
        return REDACTED_URL.into();
    };
    if url.set_username("").is_err() || url.set_password(None).is_err() {
        return REDACTED_URL.into();
    }
    url.set_query(None);
    url.set_fragment(None);
    let projected = url.to_string();
    if projected.len() > 2048 {
        REDACTED_URL.into()
    } else {
        projected
    }
}

fn parse(value: &str) -> Option<Url> {
    if value.is_empty()
        || value.len() > 2048
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.cannot_be_a_base()
        || url.as_str().len() > 2048
    {
        return None;
    }
    Some(url)
}

fn has_userinfo_marker(value: &str) -> bool {
    value
        .find("://")
        .and_then(|start| {
            let authority = &value[start + 3..];
            let end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
            authority[..end].contains('@').then_some(())
        })
        .is_some()
}

fn invalid() -> String {
    "url must be an absolute public http or https URL without credentials, query, or fragment"
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_or_secret_bearing_urls() {
        for value in [
            "http://example.test:99999",
            "http://[::1",
            "http://exa\0mple.test",
            "https://user:secret@example.test/mcp",
            "https://@example.test/mcp",
            "https://example.test/mcp?token=secret",
            "https://example.test/mcp#secret",
        ] {
            assert!(validate(value).is_err(), "{value}");
        }
        assert!(validate("https://example.test:8443/mcp").is_ok());
    }

    #[test]
    fn legacy_projection_strips_secrets_and_contains_invalid_urls() {
        assert_eq!(
            project("https://user:secret@example.test/mcp?token=x#y"),
            "https://example.test/mcp"
        );
        assert_eq!(project("http://[::1"), REDACTED_URL);
    }
}
