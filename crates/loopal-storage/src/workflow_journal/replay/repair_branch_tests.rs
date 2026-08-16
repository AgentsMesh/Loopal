use super::{TornTail, run};
use crate::workflow_journal::fs::{JournalLocation, OpenMode};

fn fixture() -> (tempfile::TempDir, JournalLocation, TornTail) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp
        .path()
        .join("sessions/session/workflows/wrun_test.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"record\n").unwrap();
    let location = JournalLocation::new(temp.path(), "session", "wrun_test", path.clone());
    let opened = match crate::workflow_journal::fs::open(&location, OpenMode::Read) {
        Ok(Some(opened)) => opened,
        _ => panic!("journal open failed"),
    };
    let tail = TornTail {
        path,
        good_offset: 0,
        observed_len: 7,
        identity: opened.identity,
    };
    (temp, location, tail)
}

#[test]
fn repair_rejects_a_token_for_another_path() {
    let (_temp, location, mut tail) = fixture();
    tail.path = std::path::PathBuf::from("other.jsonl");

    assert!(run(&location, tail).is_err());
}

#[test]
fn repair_rejects_changed_length_and_offset_at_or_after_eof() {
    let (_temp, location, mut changed_length) = fixture();
    changed_length.observed_len += 1;
    assert!(run(&location, changed_length).is_err());

    let (_temp, location, mut invalid_offset) = fixture();
    invalid_offset.good_offset = invalid_offset.observed_len;
    assert!(run(&location, invalid_offset).is_err());
}
