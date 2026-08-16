use loopal_protocol::WorkflowPermissionCausation;

pub(super) fn consume(
    owners: &mut super::AttemptOwners,
    causation: &WorkflowPermissionCausation,
) -> bool {
    let Some(tombstones) = owners.pre_aborted.get_mut(&causation.attempt_id) else {
        return false;
    };
    let Some(index) = tombstones.iter().position(|current| current == causation) else {
        return false;
    };
    tombstones.swap_remove(index);
    if tombstones.is_empty() {
        owners.pre_aborted.remove(&causation.attempt_id);
    }
    true
}
