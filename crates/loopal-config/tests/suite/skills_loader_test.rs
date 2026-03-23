use std::fs;

use loopal_config::scan_skills_dir;

#[test]
fn test_scan_skills_dir_basic() {
    let dir = tempfile::tempdir().unwrap();
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    fs::write(
        skills_dir.join("commit.md"),
        "---\ndescription: Generate commit\n---\nReview changes.\n$ARGUMENTS\n",
    )
    .unwrap();
    fs::write(skills_dir.join("review.md"), "Review the code.\n").unwrap();

    let skills = scan_skills_dir(&skills_dir);
    assert_eq!(skills.len(), 2);

    let commit = skills.iter().find(|s| s.name == "/commit").unwrap();
    assert_eq!(commit.description, "Generate commit");
    assert!(commit.has_arg);

    let review = skills.iter().find(|s| s.name == "/review").unwrap();
    assert_eq!(review.description, "Review the code.");
    assert!(!review.has_arg);
}

#[test]
fn test_scan_skills_dir_ignores_non_md_files() {
    let dir = tempfile::tempdir().unwrap();
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    fs::write(skills_dir.join("notes.txt"), "Not a skill").unwrap();
    fs::write(skills_dir.join("commit.md"), "A skill.\n").unwrap();

    let skills = scan_skills_dir(&skills_dir);
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "/commit");
}

#[test]
fn test_scan_skills_dir_empty() {
    let dir = tempfile::tempdir().unwrap();
    let skills = scan_skills_dir(dir.path());
    assert!(skills.is_empty());
}

#[test]
fn test_scan_skills_dir_sorted_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let skills_dir = dir.path().join("skills");
    fs::create_dir_all(&skills_dir).unwrap();

    fs::write(skills_dir.join("zebra.md"), "Z skill.\n").unwrap();
    fs::write(skills_dir.join("alpha.md"), "A skill.\n").unwrap();

    let skills = scan_skills_dir(&skills_dir);
    assert_eq!(skills[0].name, "/alpha");
    assert_eq!(skills[1].name, "/zebra");
}

#[test]
fn test_scan_skills_dir_missing_dir() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nonexistent");
    let skills = scan_skills_dir(&missing);
    assert!(skills.is_empty());
}
