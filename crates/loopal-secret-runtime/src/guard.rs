pub const WIRE_REF_MARKER: &str = "<secret_ref:";

pub const SECRET_REJECTION_MESSAGE: &str = "this tool rejects inputs containing <secret_ref:NAME> placeholders. \
     Secrets must NEVER be written to files via Write/Edit. \
     Use Bash with `env` injection instead, e.g. \
     { \"command\": \"echo $TOKEN\", \"env\": { \"TOKEN\": \"<secret_ref:NAME>\" } }.";

pub fn input_contains_secret_ref(input: &serde_json::Value) -> bool {
    fn walk(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::String(s) => s.contains(WIRE_REF_MARKER),
            serde_json::Value::Array(a) => a.iter().any(walk),
            serde_json::Value::Object(m) => m.values().any(walk),
            _ => false,
        }
    }
    walk(input)
}
