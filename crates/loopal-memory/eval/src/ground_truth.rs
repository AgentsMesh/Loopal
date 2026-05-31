use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::fixture::GROUND_TRUTH_YAML;

#[derive(Debug, Deserialize)]
pub struct GroundTruthFile {
    pub queries: Vec<QuerySpec>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct QuerySpec {
    pub id: String,
    pub description: String,
    pub mode: QueryMode,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub anchors: Vec<String>,
    pub relevant: Vec<RelevantItem>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QueryMode {
    Query,
    Anchor,
    Mixed,
}

impl QueryMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Anchor => "anchor",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RelevantItem {
    pub id: String,
    pub relevance: u32,
}

impl QuerySpec {
    pub fn relevant_ids(&self) -> HashSet<String> {
        self.relevant.iter().map(|r| r.id.clone()).collect()
    }

    pub fn relevance_map(&self) -> HashMap<String, u32> {
        self.relevant
            .iter()
            .map(|r| (r.id.clone(), r.relevance))
            .collect()
    }
}

pub fn load() -> GroundTruthFile {
    serde_yaml::from_str(GROUND_TRUTH_YAML).expect("ground_truth.yaml is malformed")
}
