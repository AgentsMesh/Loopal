use loopal_turn::TurnOutcome;

use super::{edited_identity, notification, resume_trigger, turn};

#[test]
fn completed_equivalents_block_resume_but_identity_mismatches_do_not() {
    let notification = notification();
    let recovered = turn(
        &notification,
        TurnOutcome::Cancelled {
            cause: loopal_turn::CancelledCause::CrashRecovery,
        },
    );
    assert!(resume_trigger(&[turn(&notification, TurnOutcome::Complete)]).is_none());
    let completed = turn(&notification, TurnOutcome::Complete);
    assert!(resume_trigger(&[completed, recovered.clone()]).is_none());

    let mismatches = [
        edited_identity(&notification, TurnOutcome::Complete, |session, _, _, _| {
            *session = "other-session".into();
        }),
        edited_identity(&notification, TurnOutcome::Complete, |_, run, _, _| {
            *run = "wrun_other".into();
        }),
        edited_identity(&notification, TurnOutcome::Complete, |_, _, revision, _| {
            *revision += 1;
        }),
        edited_identity(&notification, TurnOutcome::Complete, |_, _, _, digest| {
            *digest = "different-payload".into();
        }),
    ];
    for mismatch in mismatches {
        assert!(resume_trigger(&[mismatch, recovered.clone()]).is_some());
    }
}
