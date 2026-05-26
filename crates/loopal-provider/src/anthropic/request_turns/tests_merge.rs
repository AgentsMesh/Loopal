use super::*;

fn user(text: &str) -> Value {
    json!({"role": "user", "content": [{"type": "text", "text": text}]})
}
fn assistant(text: &str) -> Value {
    json!({"role": "assistant", "content": [{"type": "text", "text": text}]})
}

#[test]
fn merge_collapses_adjacent_user_msgs() {
    let mut v = vec![user("a"), user("b"), assistant("c"), user("d")];
    merge_adjacent_same_role(&mut v);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0]["role"], "user");
    assert_eq!(v[0]["content"].as_array().unwrap().len(), 2);
    assert_eq!(v[1]["role"], "assistant");
    assert_eq!(v[2]["role"], "user");
}

#[test]
fn merge_collapses_adjacent_assistant_msgs() {
    let mut v = vec![user("a"), assistant("b"), assistant("c"), user("d")];
    merge_adjacent_same_role(&mut v);
    assert_eq!(v.len(), 3);
    assert_eq!(v[1]["content"].as_array().unwrap().len(), 2);
}

#[test]
fn merge_noop_on_alternating() {
    let mut v = vec![user("a"), assistant("b"), user("c"), assistant("d")];
    let before = v.clone();
    merge_adjacent_same_role(&mut v);
    assert_eq!(v, before);
}

#[test]
fn merge_handles_empty_and_single() {
    let mut empty: Vec<Value> = vec![];
    merge_adjacent_same_role(&mut empty);
    assert!(empty.is_empty());

    let mut one = vec![user("a")];
    merge_adjacent_same_role(&mut one);
    assert_eq!(one.len(), 1);
}

#[test]
fn merge_handles_multiple_pairs_across_role_transitions() {
    let mut v = vec![
        user("a"),
        user("b"),
        assistant("c"),
        user("d"),
        user("e"),
        assistant("f"),
    ];
    merge_adjacent_same_role(&mut v);
    assert_eq!(v.len(), 4);
    assert_eq!(v[0]["role"], "user");
    assert_eq!(v[0]["content"].as_array().unwrap().len(), 2);
    assert_eq!(v[1]["role"], "assistant");
    assert_eq!(v[2]["role"], "user");
    assert_eq!(v[2]["content"].as_array().unwrap().len(), 2);
    assert_eq!(v[3]["role"], "assistant");
}
