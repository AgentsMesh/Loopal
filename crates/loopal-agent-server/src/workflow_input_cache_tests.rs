use loopal_runtime::workflow_input::WorkflowInputDisposition;

use super::DecisionCache;

#[test]
fn successful_decisions_are_bounded_and_oldest_first_evicted() {
    let mut cache = DecisionCache::new(2);
    let oldest = uuid::Uuid::new_v4();
    let retained = uuid::Uuid::new_v4();
    let newest = uuid::Uuid::new_v4();

    cache.insert(oldest, WorkflowInputDisposition::Handled);
    cache.insert(retained, WorkflowInputDisposition::Direct);
    cache.insert(newest, WorkflowInputDisposition::Handled);

    assert_eq!(cache.get(&oldest), None);
    assert_eq!(cache.get(&retained), Some(WorkflowInputDisposition::Direct));
    assert_eq!(cache.get(&newest), Some(WorkflowInputDisposition::Handled));
    assert_eq!(cache.values.len(), 2);
    assert_eq!(cache.insertion_order.len(), 2);
}

#[test]
fn duplicate_update_does_not_consume_capacity_or_evict_another_envelope() {
    let mut cache = DecisionCache::new(2);
    let first = uuid::Uuid::new_v4();
    let second = uuid::Uuid::new_v4();

    cache.insert(first, WorkflowInputDisposition::Direct);
    cache.insert(second, WorkflowInputDisposition::Handled);
    cache.insert(first, WorkflowInputDisposition::Handled);

    assert_eq!(cache.get(&first), Some(WorkflowInputDisposition::Handled));
    assert_eq!(cache.get(&second), Some(WorkflowInputDisposition::Handled));
    assert_eq!(cache.values.len(), 2);
    assert_eq!(cache.insertion_order.len(), 2);
}

#[test]
fn zero_capacity_never_retains_a_decision() {
    let mut cache = DecisionCache::new(0);
    let envelope = uuid::Uuid::new_v4();

    cache.insert(envelope, WorkflowInputDisposition::Handled);

    assert_eq!(cache.get(&envelope), None);
    assert!(cache.values.is_empty());
    assert!(cache.insertion_order.is_empty());
}

#[test]
fn missing_order_metadata_recovers_without_exceeding_capacity() {
    let mut cache = DecisionCache::new(1);
    let stale = uuid::Uuid::new_v4();
    let replacement = uuid::Uuid::new_v4();
    cache.values.insert(stale, WorkflowInputDisposition::Direct);

    cache.insert(replacement, WorkflowInputDisposition::Handled);

    assert_eq!(cache.get(&stale), None);
    assert_eq!(
        cache.get(&replacement),
        Some(WorkflowInputDisposition::Handled)
    );
    assert_eq!(cache.values.len(), 1);
}
