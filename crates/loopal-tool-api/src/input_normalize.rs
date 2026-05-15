use serde_json::Value;

pub fn strip_empty_optionals(input: &mut Value, required: &[&str]) {
    let obj = match input.as_object_mut() {
        Some(o) => o,
        None => return,
    };
    obj.retain(|key, val| {
        if val.as_str() == Some("") && !required.contains(&key.as_str()) {
            return false;
        }
        true
    });
}
