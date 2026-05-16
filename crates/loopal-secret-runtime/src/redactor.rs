use aho_corasick::{AhoCorasick, MatchKind};
use secrecy::{ExposeSecret, SecretString};

pub struct Redactor {
    matcher: Option<AhoCorasick>,
    names: Vec<String>,
}

impl Redactor {
    /// Build a redactor from `(name, secret)` pairs. All non-empty plaintext
    /// values become patterns, irrespective of length — vault doesn't gate
    /// secrets by length, so neither does redaction.
    ///
    /// reason: short secrets (PINs, TOTP codes, 4-char tokens) WILL trigger
    /// false-positive redactions of any string in tool output that happens
    /// to contain that literal sequence. That's the caller's choice: if a
    /// 4-char value is stored as a secret, every "1234" in shell output
    /// gets scrubbed. Consistent silence beats silently-not-protecting.
    pub fn from_pairs(pairs: &[(String, SecretString)]) -> Self {
        let mut filtered: Vec<(String, String)> = pairs
            .iter()
            .filter_map(|(n, v)| {
                let plain = v.expose_secret();
                if plain.is_empty() {
                    None
                } else {
                    Some((n.clone(), plain.to_string()))
                }
            })
            .collect();
        // Longest-match first so e.g. an 8-char secret that contains a 4-char
        // secret as substring routes to the 8-char's name.
        filtered.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        if filtered.is_empty() {
            return Self {
                matcher: None,
                names: Vec::new(),
            };
        }

        let patterns: Vec<String> = filtered.iter().map(|(_, v)| v.clone()).collect();
        let names: Vec<String> = filtered.into_iter().map(|(n, _)| n).collect();

        let matcher = AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .build(patterns)
            .ok();
        Self { matcher, names }
    }

    pub fn is_empty(&self) -> bool {
        self.matcher.is_none()
    }

    pub fn scan_and_redact(&self, output: &str) -> (String, Vec<String>) {
        let Some(matcher) = &self.matcher else {
            return (output.to_string(), Vec::new());
        };
        let mut out = String::with_capacity(output.len());
        let mut hit_names = Vec::new();
        let mut last_end = 0;
        for m in matcher.find_iter(output) {
            out.push_str(&output[last_end..m.start()]);
            let name = &self.names[m.pattern().as_usize()];
            out.push_str(&format!("<secret_ref:{name}>"));
            hit_names.push(name.clone());
            last_end = m.end();
        }
        out.push_str(&output[last_end..]);

        let mut seen = std::collections::HashSet::new();
        hit_names.retain(|n| seen.insert(n.clone()));
        (out, hit_names)
    }
}
