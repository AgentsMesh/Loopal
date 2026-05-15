use serde_json::{Map, Value};

pub fn normalize_schema(mut schema: Value) -> Value {
    let obj = match schema.as_object_mut() {
        Some(o) => o,
        None => return schema,
    };

    obj.remove("$schema");

    let definitions = obj
        .remove("definitions")
        .or_else(|| obj.remove("$defs"))
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    inline_refs_recursive(&mut schema, &definitions);
    strip_titles_recursive(&mut schema);
    ensure_object_properties_recursive(&mut schema);
    schema
}

fn inline_refs_recursive(value: &mut Value, definitions: &Map<String, Value>) {
    match value {
        Value::Object(map) => {
            if let Some(ref_val) = map.get("$ref").and_then(|v| v.as_str()).map(String::from)
                && let Some(resolved) = resolve_ref(&ref_val, definitions)
            {
                let mut resolved = resolved.clone();
                inline_refs_recursive(&mut resolved, definitions);
                *value = resolved;
                return;
            }
            for v in map.values_mut() {
                inline_refs_recursive(v, definitions);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                inline_refs_recursive(v, definitions);
            }
        }
        _ => {}
    }
}

fn resolve_ref<'a>(ref_path: &str, definitions: &'a Map<String, Value>) -> Option<&'a Value> {
    let name = ref_path
        .strip_prefix("#/definitions/")
        .or_else(|| ref_path.strip_prefix("#/$defs/"))?;
    definitions.get(name)
}

fn strip_titles_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("title");
            for v in map.values_mut() {
                strip_titles_recursive(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                strip_titles_recursive(v);
            }
        }
        _ => {}
    }
}

fn ensure_object_properties_recursive(value: &mut Value) {
    let Value::Object(map) = value else { return };

    let is_object_type = map
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == "object");

    if is_object_type && !map.contains_key("properties") {
        map.insert("properties".into(), Value::Object(Map::new()));
    }

    for v in map.values_mut() {
        match v {
            Value::Object(_) => ensure_object_properties_recursive(v),
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    ensure_object_properties_recursive(item);
                }
            }
            _ => {}
        }
    }
}
