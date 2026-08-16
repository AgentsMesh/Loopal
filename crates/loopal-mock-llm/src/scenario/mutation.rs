use anyhow::{Result, ensure};
use serde_json::Value;

use super::Scenario;

impl Scenario {
    pub(crate) fn append_calls(&mut self, values: Vec<Value>) -> Result<(usize, usize)> {
        ensure!(
            values.len() <= 4096usize.saturating_sub(self.calls.len()),
            "scenario has too many calls"
        );
        let appended = values.len();
        let parsed = values
            .into_iter()
            .map(|value| crate::scenario_parse::parse_call(value, self.version))
            .collect::<Result<Vec<_>>>()?;
        self.calls.extend(parsed);
        Ok((appended, self.calls.len()))
    }
}
