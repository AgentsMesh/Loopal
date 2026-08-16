use loopal_protocol::*;

use crate::workflow_support::{json_spec, text_spec};

fn validator() -> jsonschema::Validator {
    let schema = workflow_plan_schema();
    jsonschema::draft202012::meta::validate(&schema).expect("planner schema is valid JSON Schema");
    jsonschema::draft202012::options()
        .build(&schema)
        .expect("planner schema compiles");
    jsonschema::draft202012::options()
        .build(&schema)
        .expect("planner schema compiles")
}

fn workflow_value(spec: WorkflowSpec) -> serde_json::Value {
    serde_json::to_value(WorkflowPlanDecision {
        version: WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec },
    })
    .unwrap()
}

#[test]
fn canonical_schema_accepts_each_declared_planner_branch() {
    let validate = validator();
    for value in [
        serde_json::json!({"version": 1, "execution": {"kind": "direct"}}),
        serde_json::json!({"version": 1, "execution": {"kind": "direct", "reason": "small"}}),
        workflow_value(text_spec()),
        workflow_value(json_spec()),
    ] {
        assert!(validate.is_valid(&value), "schema rejected {value}");
    }
}

#[test]
fn canonical_schema_rejects_wrong_branch_and_nested_wire_shapes() {
    let validate = validator();
    let invalid = [
        serde_json::json!({"version": 1, "execution": {"kind": "workflow"}}),
        serde_json::json!({
            "version": 1,
            "execution": {"kind": "direct", "spec": serde_json::to_value(text_spec()).unwrap()}
        }),
        serde_json::json!({
            "version": 1,
            "execution": {"kind": "workflow", "reason": "forged", "spec": serde_json::to_value(text_spec()).unwrap()}
        }),
    ];
    for value in invalid {
        assert!(!validate.is_valid(&value), "schema accepted {value}");
    }

    for (path, field) in [
        (vec!["execution", "spec"], "forged"),
        (vec!["execution", "spec", "nodes", "0"], "forged"),
        (vec!["execution", "spec", "limits"], "forged"),
        (vec!["execution", "spec", "output_contract"], "forged"),
    ] {
        let mut value = workflow_value(text_spec());
        let mut target = &mut value;
        for segment in path {
            target = if let Ok(index) = segment.parse::<usize>() {
                &mut target.as_array_mut().unwrap()[index]
            } else {
                target.get_mut(segment).unwrap()
            };
        }
        target
            .as_object_mut()
            .unwrap()
            .insert(field.into(), true.into());
        assert!(!validate.is_valid(&value), "schema accepted {value}");
    }
}

#[test]
fn canonical_schema_rejects_invalid_ids_limits_and_json_contracts() {
    let validate = validator();
    let mut bad_id = workflow_value(text_spec());
    bad_id["execution"]["spec"]["nodes"][0]["id"] = "_bad".into();
    assert!(!validate.is_valid(&bad_id));

    let mut bad_limit = workflow_value(text_spec());
    bad_limit["execution"]["spec"]["limits"]["max_nodes"] = 0.into();
    assert!(!validate.is_valid(&bad_limit));

    let mut bad_json_contract = workflow_value(json_spec());
    bad_json_contract["execution"]["spec"]["output_contract"]["schema"] = true.into();
    assert!(!validate.is_valid(&bad_json_contract));
}
