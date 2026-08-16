use serde_json::Value;

use super::{MockCall, Scenario};

impl Scenario {
    pub fn take_matching(
        &mut self,
        body: &Value,
        record: &crate::RequestRecord,
    ) -> (Option<MockCall>, Vec<String>) {
        let mut failures = Vec::new();
        let position = self.calls.iter().position(|call| {
            if call.expected.is_empty() {
                return false;
            }
            let errors = crate::validate_request(&call.expected, body, record);
            if !errors.is_empty() {
                failures.push(candidate_failure(call, &errors));
            }
            errors.is_empty()
        });
        let position =
            position.or_else(|| self.calls.iter().position(|call| call.expected.is_empty()));
        let call = position
            .and_then(|index| self.calls.remove(index))
            .or_else(|| self.fallback.clone());
        if call.is_some() {
            self.served += 1;
        }
        (call, failures)
    }
}

fn candidate_failure(call: &MockCall, errors: &[String]) -> String {
    let candidate = call.label.as_deref().unwrap_or("<unlabeled>");
    format!("candidate {candidate}: {}", errors.join("; "))
}
