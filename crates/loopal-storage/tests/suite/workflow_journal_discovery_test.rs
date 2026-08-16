use loopal_storage::{
    MAX_WORKFLOW_JOURNALS_PER_SESSION, MAX_WORKFLOW_SESSION_JOURNAL_BYTES, SessionStore,
    WorkflowJournalError, WorkflowJournalLimit,
};

fn session_store(temp: &tempfile::TempDir) -> SessionStore {
    SessionStore::with_base_dir(temp.path().to_path_buf())
}

fn workflow_directory(temp: &tempfile::TempDir) -> std::path::PathBuf {
    temp.path().join("sessions/session-one/workflows")
}

#[test]
fn missing_directory_is_empty_and_results_are_sorted() {
    let temp = tempfile::tempdir().unwrap();
    let store = session_store(&temp);
    assert!(
        store
            .list_workflow_run_ids("session-one")
            .unwrap()
            .is_empty()
    );

    let directory = workflow_directory(&temp);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("wrun_z.jsonl"), []).unwrap();
    std::fs::write(directory.join("wrun_a.jsonl"), []).unwrap();
    assert_eq!(
        store.list_workflow_run_ids("session-one").unwrap(),
        vec!["wrun_a".into(), "wrun_z".into()]
    );
}

#[test]
fn invalid_session_filename_and_non_file_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let store = session_store(&temp);
    assert!(store.list_workflow_run_ids("../session").is_err());

    for name in ["bad/id.jsonl", "bad.txt", "bad!.jsonl"] {
        let temp = tempfile::tempdir().unwrap();
        let directory = workflow_directory(&temp);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, []).unwrap();
        assert!(matches!(
            session_store(&temp).list_workflow_run_ids("session-one"),
            Err(WorkflowJournalError::Corruption { .. })
        ));
    }

    let temp = tempfile::tempdir().unwrap();
    let directory = workflow_directory(&temp);
    std::fs::create_dir_all(directory.join("wrun_directory.jsonl")).unwrap();
    assert!(matches!(
        session_store(&temp).list_workflow_run_ids("session-one"),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[cfg(unix)]
#[test]
fn symlink_journal_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let directory = workflow_directory(&temp);
    std::fs::create_dir_all(&directory).unwrap();
    let target = temp.path().join("target");
    std::fs::write(&target, []).unwrap();
    std::os::unix::fs::symlink(target, directory.join("wrun_link.jsonl")).unwrap();
    assert!(matches!(
        session_store(&temp).list_workflow_run_ids("session-one"),
        Err(WorkflowJournalError::Corruption { .. })
    ));
}

#[test]
fn journal_count_and_session_bytes_are_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let directory = workflow_directory(&temp);
    std::fs::create_dir_all(&directory).unwrap();
    for index in 0..=MAX_WORKFLOW_JOURNALS_PER_SESSION {
        std::fs::write(directory.join(format!("wrun_{index}.jsonl")), []).unwrap();
    }
    assert!(matches!(
        session_store(&temp).list_workflow_run_ids("session-one"),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::Journals,
            ..
        })
    ));

    let temp = tempfile::tempdir().unwrap();
    let directory = workflow_directory(&temp);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::File::create(directory.join("wrun_large.jsonl"))
        .unwrap()
        .set_len(MAX_WORKFLOW_SESSION_JOURNAL_BYTES + 1)
        .unwrap();
    assert!(matches!(
        session_store(&temp).list_workflow_run_ids("session-one"),
        Err(WorkflowJournalError::LimitExceeded {
            limit: WorkflowJournalLimit::SessionBytes,
            ..
        })
    ));
}
