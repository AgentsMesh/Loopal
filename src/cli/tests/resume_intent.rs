use clap::Parser;

use super::*;

#[test]
fn none_when_unset() {
    let cli = Cli::parse_from(["loopal"]);
    assert!(cli.resume_intent().is_none());
}

#[test]
fn latest_when_bare() {
    let cli = Cli::parse_from(["loopal", "--resume"]);
    assert!(matches!(cli.resume_intent(), Some(ResumeIntent::Latest)));
}

#[test]
fn specific_when_id_given() {
    let cli = Cli::parse_from(["loopal", "--resume", "abc-123"]);
    match cli.resume_intent() {
        Some(ResumeIntent::Specific(id)) => assert_eq!(id, "abc-123"),
        other => panic!("expected Specific, got {other:?}"),
    }
}
