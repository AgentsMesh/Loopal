use super::{A, gate, hits, manifest, parse, record};

#[test]
fn generated_bodies_do_not_mask_the_function() {
    let generic = "_RINvNtC4demo8criticalNtC4test10AcceptJsonEB1_";
    let closure = "_RNCNvNtC4demo8critical0B1_";
    let coroutine = "_RNCINvNtC4demo8criticalNtC4testE0B1_";
    let nested = "_RNCNCNvNtC4demo8critical00B1_";
    let same_named_module = "_RNvNtC4demo8critical6helper";
    let text = record(
        A,
        &hits(100, 4),
        &[
            (generic, 1),
            (closure, 1),
            (coroutine, 1),
            (nested, 1),
            (same_named_module, 1),
        ],
        &hits(10, 1),
        None,
    );
    let sources = manifest(&[A]);
    assert!(gate::evaluate(&parse(&text, &sources), &sources).is_ok());
}

#[test]
fn same_named_module_matches_the_function_segment() {
    let body = "_RNvNtNtC4demo8critical8critical";
    let helper = "_RNvNtNtC4demo8critical6helper";
    let text = record(
        A,
        &hits(100, 4),
        &[(body, 1), (helper, 1)],
        &hits(10, 1),
        None,
    );
    let sources = manifest(&[A]);
    assert!(gate::evaluate(&parse(&text, &sources), &sources).is_ok());
}

#[test]
fn concrete_generic_instances_match_their_function_segment() {
    let uncovered = concat!(
        "_RNvXs_NtC15loopal_tool_api12typed_bridgeI",
        "NtB4_11TypedBridgeNtC8test_toolE",
        "NtNtB6_4tool4Tool19image_output_policyC5tools"
    );
    let covered = concat!(
        "_RNvXs_NtC15loopal_tool_api12typed_bridgeI",
        "NtB4_11TypedBridgeNtC9other_toolE",
        "NtNtB6_4tool4Tool19image_output_policyC5tools"
    );
    let text = record(
        A,
        &hits(100, 4),
        &[(uncovered, 0), (covered, 2)],
        &hits(10, 1),
        None,
    )
    .replace(&format!("FN:2,{covered}"), &format!("FN:1,{covered}"));
    let mut sources = manifest(&[A]);
    sources.critical[0].name = "image_output_policy".into();
    assert!(gate::evaluate(&parse(&text, &sources), &sources).is_ok());
}
