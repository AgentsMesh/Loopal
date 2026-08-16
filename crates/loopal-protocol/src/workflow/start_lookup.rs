use serde::{Deserialize, Deserializer, Serialize};

use super::{WorkflowRequestId, WorkflowStartResponse};

/// Read-only lookup for the durable result of a prior workflow start request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStartLookupRequest {
    pub request_id: WorkflowRequestId,
}

/// A lookup never creates a ledger record. `Conflict` means the stable request
/// id is already owned by a different workflow operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowStartLookupResponse {
    NotFound,
    Found { response: WorkflowStartResponse },
    Conflict,
}

impl<'de> Deserialize<'de> for WorkflowStartLookupResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("lookup response must be an object"))?;
        let status = object
            .get("status")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| serde::de::Error::custom("lookup response status is required"))?;
        match status {
            "not_found" if object.len() == 1 => Ok(Self::NotFound),
            "conflict" if object.len() == 1 => Ok(Self::Conflict),
            "found" if object.len() == 2 => {
                let response = object
                    .get("response")
                    .cloned()
                    .ok_or_else(|| serde::de::Error::custom("found response is required"))?;
                serde_json::from_value(response)
                    .map(|response| Self::Found { response })
                    .map_err(serde::de::Error::custom)
            }
            "not_found" | "conflict" | "found" => Err(serde::de::Error::custom(
                "lookup response contains unknown or missing fields",
            )),
            _ => Err(serde::de::Error::custom("unknown lookup response status")),
        }
    }
}
