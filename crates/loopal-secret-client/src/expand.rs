use regex::Regex;
use secrecy::{ExposeSecret, SecretString};

use crate::error::{SecretError, SecretResult};

pub fn collect_names(re: &Regex, input: &str) -> Vec<String> {
    re.captures_iter(input).map(|c| c[1].to_string()).collect()
}

/// Expand `re` placeholders in `input` by calling `fetch` for each unique
/// captured name. Returns `SecretNotFound` on the first missing name.
///
/// Walks matches manually instead of `replace_all` + closure: the closure
/// would have to return `String`, which means each substitution leaks a
/// short-lived plaintext copy onto the heap that `regex` then `push_str`s
/// into its internal buffer — that intermediate `String`'s allocator drop
/// will not zeroize. Manual `push_str` from the resolved `SecretString`
/// keeps plaintext in exactly one buffer (`buf`), whose ownership transfers
/// to `SecretString` for zeroize-on-drop.
pub async fn expand_template<F, Fut>(
    re: &Regex,
    input: &str,
    mut fetch: F,
) -> SecretResult<SecretString>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = SecretResult<SecretString>>,
{
    let names = collect_names(re, input);
    if names.is_empty() {
        return Ok(SecretString::from(input.to_string()));
    }
    let mut resolved: std::collections::HashMap<String, SecretString> =
        std::collections::HashMap::new();
    for n in &names {
        if !resolved.contains_key(n) {
            let v = fetch(n.clone()).await?;
            resolved.insert(n.clone(), v);
        }
    }
    let mut buf = String::with_capacity(input.len());
    let mut cursor = 0usize;
    for caps in re.captures_iter(input) {
        let m = caps.get(0).expect("regex match has full capture");
        buf.push_str(&input[cursor..m.start()]);
        let name = &caps[1];
        match resolved.get(name) {
            Some(v) => buf.push_str(v.expose_secret()),
            None => return Err(SecretError::SecretNotFound(name.to_string())),
        }
        cursor = m.end();
    }
    buf.push_str(&input[cursor..]);
    Ok(SecretString::from(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placeholder::{AUTHOR_RE, WIRE_RE};

    fn make_fetch(
        map: std::collections::HashMap<&'static str, &'static str>,
    ) -> impl Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = SecretResult<SecretString>> + Send>>
    {
        move |name: String| {
            let map = map.clone();
            Box::pin(async move {
                match map.get(name.as_str()) {
                    Some(v) => Ok(SecretString::from((*v).to_string())),
                    None => Err(SecretError::SecretNotFound(name)),
                }
            })
        }
    }

    #[test]
    fn collect_names_author() {
        let names = collect_names(&AUTHOR_RE, "a {{secret:k1}} b {{secret:k2}}");
        assert_eq!(names, vec!["k1", "k2"]);
    }

    #[test]
    fn collect_names_empty_when_no_placeholder() {
        assert!(collect_names(&AUTHOR_RE, "plain text").is_empty());
    }

    #[tokio::test]
    async fn substitutes_unique_placeholders() {
        let fetch = make_fetch(std::collections::HashMap::from([
            ("api_key", "sk-abc"),
            ("base", "https://api.example"),
        ]));
        let out = expand_template(
            &AUTHOR_RE,
            "url={{secret:base}}/v1 key={{secret:api_key}}",
            fetch,
        )
        .await
        .unwrap();
        assert_eq!(out.expose_secret(), "url=https://api.example/v1 key=sk-abc");
    }

    #[tokio::test]
    async fn passes_through_input_without_placeholders() {
        let fetch = make_fetch(std::collections::HashMap::new());
        let out = expand_template(&AUTHOR_RE, "no placeholders here", fetch)
            .await
            .unwrap();
        assert_eq!(out.expose_secret(), "no placeholders here");
    }

    #[tokio::test]
    async fn returns_not_found_on_missing_secret() {
        let fetch = make_fetch(std::collections::HashMap::new());
        let err = expand_template(&AUTHOR_RE, "{{secret:absent}}", fetch)
            .await
            .unwrap_err();
        assert!(matches!(err, SecretError::SecretNotFound(n) if n == "absent"));
    }

    #[tokio::test]
    async fn duplicate_names_fetched_once() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count_for_fetch = count.clone();
        let fetch = move |name: String| {
            let c = count_for_fetch.clone();
            Box::pin(async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(SecretString::from(format!("V({name})")))
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = SecretResult<SecretString>> + Send>>
        };
        let out = expand_template(&WIRE_RE, "<secret_ref:k><secret_ref:k>", fetch)
            .await
            .unwrap();
        assert_eq!(out.expose_secret(), "V(k)V(k)");
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
