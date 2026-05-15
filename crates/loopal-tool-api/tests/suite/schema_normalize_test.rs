use serde_json::json;

use loopal_tool_api::schema_normalize::normalize_schema;

#[test]
fn removes_dollar_schema() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {"x": {"type": "string"}}
    });
    let result = normalize_schema(schema);
    assert!(result.get("$schema").is_none());
}

#[test]
fn removes_title_at_all_levels() {
    let schema = json!({
        "title": "MyParams",
        "type": "object",
        "properties": {
            "name": {"title": "Name", "type": "string"}
        }
    });
    let result = normalize_schema(schema);
    assert!(result.get("title").is_none());
    assert!(result["properties"]["name"].get("title").is_none());
    assert_eq!(result["properties"]["name"]["type"], "string");
}

#[test]
fn inlines_ref_from_definitions() {
    let schema = json!({
        "type": "object",
        "properties": {
            "item": {"$ref": "#/definitions/Item"}
        },
        "definitions": {
            "Item": {
                "type": "object",
                "properties": {"id": {"type": "integer"}}
            }
        }
    });
    let result = normalize_schema(schema);
    assert!(result.get("definitions").is_none());
    let item = &result["properties"]["item"];
    assert!(item.get("$ref").is_none());
    assert_eq!(item["type"], "object");
    assert_eq!(item["properties"]["id"]["type"], "integer");
}

#[test]
fn inlines_ref_from_defs() {
    let schema = json!({
        "type": "object",
        "properties": {
            "item": {"$ref": "#/$defs/Item"}
        },
        "$defs": {
            "Item": {"type": "string"}
        }
    });
    let result = normalize_schema(schema);
    assert!(result.get("$defs").is_none());
    assert_eq!(result["properties"]["item"]["type"], "string");
}

#[test]
fn inlines_nested_refs() {
    let schema = json!({
        "type": "object",
        "properties": {
            "outer": {"$ref": "#/definitions/Outer"}
        },
        "definitions": {
            "Outer": {
                "type": "object",
                "properties": {
                    "inner": {"$ref": "#/definitions/Inner"}
                }
            },
            "Inner": {"type": "number"}
        }
    });
    let result = normalize_schema(schema);
    let inner = &result["properties"]["outer"]["properties"]["inner"];
    assert!(inner.get("$ref").is_none());
    assert_eq!(inner["type"], "number");
}

#[test]
fn inlines_ref_in_array_items() {
    let schema = json!({
        "type": "object",
        "properties": {
            "list": {
                "type": "array",
                "items": {"$ref": "#/definitions/Entry"}
            }
        },
        "definitions": {
            "Entry": {"type": "string"}
        }
    });
    let result = normalize_schema(schema);
    assert_eq!(result["properties"]["list"]["items"]["type"], "string");
}

#[test]
fn preserves_non_ref_schema_structure() {
    let schema = json!({
        "type": "object",
        "required": ["command"],
        "properties": {
            "command": {"type": "string"},
            "timeout": {"type": "integer"}
        }
    });
    let result = normalize_schema(schema.clone());
    assert_eq!(result, schema);
}

#[test]
fn handles_unresolvable_ref_gracefully() {
    let schema = json!({
        "type": "object",
        "properties": {
            "x": {"$ref": "#/definitions/Missing"}
        },
        "definitions": {}
    });
    let result = normalize_schema(schema);
    assert_eq!(result["properties"]["x"]["$ref"], "#/definitions/Missing");
}
