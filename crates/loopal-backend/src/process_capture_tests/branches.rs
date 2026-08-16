use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::process_capture_buffers::{CappedText, HeadTail, Tail};
use crate::process_capture_frame::{Framer, Utf8Decoder};
use crate::process_capture_preview::{CaptureSource, ProcessPreviews};
use crate::process_capture_render::render_preview;
use crate::process_capture_state::ProcessCaptureState;

#[test]
fn head_tail_renders_elision_without_a_leading_separator() {
    let mut lines = HeadTail::new(0, 1);
    lines.push(Zeroizing::new("hidden".into()));
    lines.push(Zeroizing::new("latest".into()));
    assert_eq!(&*lines.render(), "[... 1 lines elided ...]\nlatest");
}

#[test]
fn bounded_tail_discards_its_oldest_line() {
    let mut lines = Tail::new(1);
    lines.push(Zeroizing::new("old".into()));
    lines.push(Zeroizing::new("new".into()));
    assert_eq!(&*lines.render(), "new");
}

#[test]
fn capped_text_trims_on_a_utf8_boundary() {
    let mut text = CappedText::new();
    text.push(&"\u{754c}".repeat(3_073));
    let snapshot = text.snapshot();
    assert!(text.truncated);
    assert!(snapshot.len() <= 8 * 1_024);
    assert!(std::str::from_utf8(snapshot.as_bytes()).is_ok());
}

#[test]
fn stderr_framer_preserves_cross_chunk_line_state() {
    let mut framer = Framer::new();
    assert_eq!(
        &*framer.frame(CaptureSource::Stderr, Zeroizing::new(b"part".to_vec())),
        b"[err] part"
    );
    assert_eq!(
        &*framer.frame(CaptureSource::Stderr, Zeroizing::new(b"ial\n".to_vec())),
        b"ial\n"
    );
}

#[test]
fn utf8_decoder_buffers_and_finishes_an_incomplete_scalar() {
    let mut decoder = Utf8Decoder::new();
    assert!(decoder.push(&[0xe2, 0x82]).is_empty());
    assert_eq!(&*decoder.finish(), "\u{fffd}".as_bytes());
}

#[test]
fn empty_preview_lines_are_ignored() {
    let mut previews = ProcessPreviews::new(1);
    previews.absorb(CaptureSource::Stdout, b"\n");
    previews.finish(CaptureSource::Stdout);
    let rendered = previews.render();
    assert!(rendered.stdout.is_empty());
    assert!(rendered.progress.is_empty());
}

#[test]
fn renderer_labels_truncated_stderr() {
    let rendered = render_preview(
        "out",
        false,
        "err",
        true,
        std::path::Path::new("/private/log"),
    );
    assert!(rendered.contains("[stderr, truncated to last 8 KB]"));
}

#[test]
fn capture_state_renders_both_nonterminal_statuses() {
    let state = ProcessCaptureState::new(PathBuf::from("/private/log"), None);
    assert!(state.render_output(false).contains("[Status: Running]"));
    assert!(
        state
            .render_output(true)
            .contains("[Status: Running (timed out waiting)]")
    );
}
