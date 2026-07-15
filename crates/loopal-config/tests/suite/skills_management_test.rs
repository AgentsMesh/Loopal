use std::fs;

use loopal_config::{
    delete_global_skill, get_global_skill, list_skill_documents, scan_skills_dir,
    upsert_global_skill,
};

#[test]
fn global_skill_crud_uses_exact_revisions() {
    let root = tempfile::tempdir().unwrap();
    let created = upsert_global_skill(
        root.path(),
        "/desktop-check",
        "Desktop check",
        "Verify $ARGUMENTS",
        None,
    )
    .unwrap();
    assert_eq!(created.revision.len(), 64);
    assert_eq!(created.skill.description, "Desktop check");
    assert!(created.skill.has_arg);
    let path = root.path().join("skills/desktop-check.md");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "---\ndescription: Desktop check\n---\nVerify $ARGUMENTS"
    );
    assert!(
        fs::read_dir(root.path().join("skills"))
            .unwrap()
            .flatten()
            .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp."))
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(
        upsert_global_skill(root.path(), "/desktop-check", "Lost update", "body", None,).is_err()
    );

    let updated = upsert_global_skill(
        root.path(),
        "/desktop-check",
        "Updated",
        "Exact body",
        Some(&created.revision),
    )
    .unwrap();
    assert_ne!(updated.revision, created.revision);
    assert!(delete_global_skill(root.path(), "/desktop-check", &created.revision).is_err());
    delete_global_skill(root.path(), "/desktop-check", &updated.revision).unwrap();
    assert!(get_global_skill(root.path(), "/desktop-check").is_err());
}

#[test]
fn concurrent_updates_allow_exactly_one_revision_winner() {
    use std::sync::{Arc, Barrier};
    let root = tempfile::tempdir().unwrap();
    let created = upsert_global_skill(root.path(), "/race", "Initial", "body", None).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let workers = ["first", "second"].map(|body| {
        let root = root.path().to_path_buf();
        let revision = created.revision.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            upsert_global_skill(&root, "/race", "Updated", body, Some(&revision))
        })
    });
    barrier.wait();
    let results = workers.map(|worker| worker.join().unwrap());
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert_ne!(
        get_global_skill(root.path(), "/race").unwrap().revision,
        created.revision
    );
}

#[test]
fn managed_skill_validation_preserves_body_semantics() {
    let root = tempfile::tempdir().unwrap();
    for name in ["desktop-check", "/../escape", "/bad.name", "/技能"] {
        assert!(upsert_global_skill(root.path(), name, "Valid", "body", None).is_err());
    }
    assert!(upsert_global_skill(root.path(), "/ok", "", "body", None).is_err());
    assert!(upsert_global_skill(root.path(), "/ok", "bad\nheader", "body", None).is_err());
    assert!(upsert_global_skill(root.path(), "/ok", "Valid", "bad\0body", None).is_err());
    assert!(upsert_global_skill(root.path(), "/ok", "Valid", &"x".repeat(102_400), None).is_err());

    let skill = upsert_global_skill(root.path(), "/ok", "Valid", "\n body \n", None).unwrap();
    assert_eq!(skill.skill.body, "\n body \n");
}

#[test]
fn oversized_skills_are_not_loaded() {
    let root = tempfile::tempdir().unwrap();
    let skills = root.path().join("skills");
    fs::create_dir(&skills).unwrap();
    fs::write(skills.join("large.md"), vec![b'x'; 102_401]).unwrap();
    assert!(scan_skills_dir(&skills).is_empty());
}

#[cfg(unix)]
#[test]
fn readonly_symlink_skill_stays_visible_but_cannot_be_managed() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside.md");
    let skills = root.path().join("skills");
    fs::create_dir(&skills).unwrap();
    fs::write(&outside, "Linked body").unwrap();
    symlink(&outside, skills.join("linked.md")).unwrap();

    let listed = list_skill_documents(&skills).unwrap();
    assert_eq!(listed[0].skill.name, "/linked");
    assert!(!listed[0].direct_regular_file);
    assert_eq!(scan_skills_dir(&skills)[0].name, "/linked");
    assert!(get_global_skill(root.path(), "/linked").is_err());
    assert!(upsert_global_skill(root.path(), "/linked", "Valid", "body", None).is_err());
}
