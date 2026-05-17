use loopal_tool_api::{HeadTail, StderrCappedBuffer};

#[test]
fn head_tail_under_capacity_shows_all_lines() {
    let ht = HeadTail::new(5, 5);
    for i in 0..3 {
        ht.push_line(format!("line {i}"));
    }
    let preview = ht.render_preview();
    assert!(preview.contains("line 0"));
    assert!(preview.contains("line 2"));
    assert!(!preview.contains("elided"));
    assert!(!ht.was_truncated());
}

#[test]
fn head_tail_exact_capacity_no_elision() {
    let ht = HeadTail::new(2, 3);
    for i in 0..5 {
        ht.push_line(format!("L{i}"));
    }
    let preview = ht.render_preview();
    assert!(preview.contains("L0"));
    assert!(preview.contains("L4"));
    assert!(!preview.contains("elided"));
    assert!(!ht.was_truncated());
}

#[test]
fn head_tail_overflow_elides_middle() {
    let ht = HeadTail::new(2, 2);
    for i in 0..10 {
        ht.push_line(format!("L{i}"));
    }
    let preview = ht.render_preview();
    assert!(preview.starts_with("L0\nL1"));
    assert!(preview.ends_with("L8\nL9"));
    assert!(preview.contains("[... 6 lines elided ...]"));
    assert!(ht.was_truncated());
    assert_eq!(ht.total_lines(), 10);
}

#[test]
fn head_tail_utf8_multibyte_lines_preserved() {
    let ht = HeadTail::new(2, 2);
    ht.push_line("中文头一".into());
    ht.push_line("中文头二".into());
    for _ in 0..50 {
        ht.push_line("filler".into());
    }
    ht.push_line("中文尾一".into());
    ht.push_line("中文尾二".into());
    let preview = ht.render_preview();
    assert!(preview.contains("中文头一"));
    assert!(preview.contains("中文尾二"));
}

#[test]
fn stderr_buf_short_input_no_trim() {
    let mut sb = StderrCappedBuffer::new();
    sb.push_str("warning: foo\n");
    sb.push_str("error: bar\n");
    assert_eq!(sb.snapshot(), "warning: foo\nerror: bar\n");
    assert!(!sb.was_truncated());
}

#[test]
fn stderr_buf_overflow_trims_front() {
    let mut sb = StderrCappedBuffer::new();
    let big_line = "x".repeat(2048);
    for _ in 0..10 {
        sb.push_str(&big_line);
        sb.push_str("\n");
    }
    sb.push_str("LAST_LINE\n");
    let snap = sb.snapshot();
    assert!(sb.was_truncated());
    assert!(snap.ends_with("LAST_LINE\n"));
    assert!(snap.len() <= 10 * 1024); // within cap + headroom
}

#[test]
fn stderr_buf_trim_respects_utf8_boundary() {
    let mut sb = StderrCappedBuffer::new();
    let multibyte = "中".repeat(5000); // 15 KB, well over cap+headroom
    sb.push_str(&multibyte);
    sb.push_str("\n");
    let snap = sb.snapshot();
    // Snapshot must be valid UTF-8 (already enforced by String type)
    assert!(sb.was_truncated());
    assert!(snap.ends_with('\n'));
}

#[test]
fn stderr_buf_default_is_empty() {
    let sb = StderrCappedBuffer::default();
    assert!(sb.is_empty());
    assert!(!sb.was_truncated());
}
