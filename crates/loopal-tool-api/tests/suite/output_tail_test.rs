use loopal_tool_api::OutputTail;

#[test]
fn push_line_keeps_only_the_configured_tail() {
    let tail = OutputTail::new(2);
    tail.push_line("first".into());
    tail.push_line("second".into());
    tail.push_line("third".into());

    assert_eq!(tail.snapshot(), "second\nthird");
}

#[test]
fn snapshot_replaces_lines_until_the_next_push() {
    let tail = OutputTail::new(2);
    tail.push_line("line".into());
    tail.replace_snapshot("whole snapshot".into());
    assert_eq!(tail.snapshot(), "whole snapshot");

    tail.push_line("next".into());
    assert_eq!(tail.snapshot(), "next");
}

#[test]
fn zero_capacity_retains_no_lines() {
    let tail = OutputTail::new(0);
    tail.push_line("discarded".into());
    assert!(tail.snapshot().is_empty());
}
