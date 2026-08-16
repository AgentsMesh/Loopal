use secrecy::{ExposeSecret, SecretString};

struct Pattern {
    name: String,
    value: SecretString,
}

pub struct Redactor {
    patterns: Vec<Pattern>,
}

impl Redactor {
    pub fn from_pairs(pairs: &[(String, SecretString)]) -> Self {
        let mut patterns: Vec<_> = pairs
            .iter()
            .filter(|(_, value)| !value.expose_secret().is_empty())
            .map(|(name, value)| Pattern {
                name: name.clone(),
                value: value.clone(),
            })
            .collect();
        patterns.sort_by(|left, right| {
            right
                .value
                .expose_secret()
                .len()
                .cmp(&left.value.expose_secret().len())
        });
        Self { patterns }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn scan_and_redact(&self, output: &str) -> (String, Vec<String>) {
        if self.patterns.is_empty() {
            return (output.to_string(), Vec::new());
        }
        let mut redacted = String::with_capacity(output.len());
        let mut hit_names = Vec::new();
        let mut cursor = 0;
        let mut copied_until = 0;
        while cursor < output.len() {
            let Some(pattern) = self
                .patterns
                .iter()
                .find(|pattern| output[cursor..].starts_with(pattern.value.expose_secret()))
            else {
                cursor += output[cursor..]
                    .chars()
                    .next()
                    .expect("cursor is before end")
                    .len_utf8();
                continue;
            };
            redacted.push_str(&output[copied_until..cursor]);
            redacted.push_str("<secret_ref:");
            redacted.push_str(&pattern.name);
            redacted.push('>');
            hit_names.push(pattern.name.clone());
            cursor += pattern.value.expose_secret().len();
            copied_until = cursor;
        }
        if hit_names.is_empty() {
            return (output.to_string(), hit_names);
        }
        redacted.push_str(&output[copied_until..]);
        let mut seen = std::collections::HashSet::new();
        hit_names.retain(|name| seen.insert(name.clone()));
        (redacted, hit_names)
    }
}
