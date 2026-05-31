use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallSource {
    DirectHit,
    Neighbor,
    CoOccur,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    QueryEvent {
        qid: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        query: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        anchor: Vec<String>,
        result_count: u32,
        latency_ms: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caller: Option<String>,
    },
    RecallHit {
        qid: String,
        node: String,
        rank: u32,
        score: f32,
        source: RecallSource,
    },
    ImportanceTag {
        node: String,
        importance: i8,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u8,
    pub ts: i64,
    pub sid: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

impl Event {
    pub fn new(sid: impl Into<String>, ts: i64, kind: EventKind) -> Self {
        Self {
            v: 1,
            ts,
            sid: sid.into(),
            kind,
        }
    }

    pub fn node_slug(&self) -> Option<&str> {
        match &self.kind {
            EventKind::RecallHit { node, .. } | EventKind::ImportanceTag { node, .. } => {
                Some(node.as_str())
            }
            _ => None,
        }
    }
}
