use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;

use loopal_secret_client::placeholder::WIRE_RE;

#[derive(Debug, Default)]
pub struct ResolverReport {
    pub resolved_names: Vec<String>,
    pub missing: Vec<String>,
}

pub fn collect_wire_refs(value: &Value, whitelist: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    walk_collect(value, whitelist, &mut out, false);
    let mut seen = std::collections::HashSet::new();
    out.retain(|n| seen.insert(n.clone()));
    out
}

fn walk_collect(value: &Value, whitelist: &[&str], out: &mut Vec<String>, in_eligible: bool) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let eligible = in_eligible || whitelist.contains(&k.as_str());
                walk_collect(v, whitelist, out, eligible);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                walk_collect(v, whitelist, out, in_eligible);
            }
        }
        Value::String(s) if in_eligible => {
            for cap in WIRE_RE.captures_iter(s) {
                out.push(cap[1].to_string());
            }
        }
        _ => {}
    }
}

pub fn resolve_in_value(
    value: &mut Value,
    secrets: &HashMap<String, SecretString>,
    whitelist: &[&str],
) -> ResolverReport {
    let mut report = ResolverReport::default();
    walk_resolve(value, whitelist, secrets, &mut report, false);
    let mut seen = std::collections::HashSet::new();
    report.resolved_names.retain(|n| seen.insert(n.clone()));
    let mut seen_m = std::collections::HashSet::new();
    report.missing.retain(|n| seen_m.insert(n.clone()));
    report
}

fn walk_resolve(
    value: &mut Value,
    whitelist: &[&str],
    secrets: &HashMap<String, SecretString>,
    report: &mut ResolverReport,
    in_eligible: bool,
) {
    match value {
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                let eligible = in_eligible || whitelist.contains(&k.as_str());
                walk_resolve(v, whitelist, secrets, report, eligible);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                walk_resolve(v, whitelist, secrets, report, in_eligible);
            }
        }
        Value::String(s) if in_eligible => {
            *s = substitute_in_string(s, secrets, report);
        }
        _ => {}
    }
}

fn substitute_in_string(
    s: &str,
    secrets: &HashMap<String, SecretString>,
    report: &mut ResolverReport,
) -> String {
    WIRE_RE
        .replace_all(s, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            if let Some(v) = secrets.get(name) {
                report.resolved_names.push(name.to_string());
                v.expose_secret().to_string()
            } else {
                report.missing.push(name.to_string());
                format!("<missing-secret:{name}>")
            }
        })
        .into_owned()
}
