use serde::{Deserialize, Serialize};

use loopal_error::MemoryGraphError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    User,
    Feedback,
    Project,
    Reference,
    Index,
}

impl MemoryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
            Self::Index => "index",
        }
    }

    pub fn parse(s: &str) -> Result<Self, MemoryGraphError> {
        match s {
            "user" => Ok(Self::User),
            "feedback" => Ok(Self::Feedback),
            "project" => Ok(Self::Project),
            "reference" => Ok(Self::Reference),
            "index" => Ok(Self::Index),
            other => Err(MemoryGraphError::InvalidNodeKind(other.into())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    References,
    ContainedIn,
    SupersededBy,
    DerivedFrom,
    CoOccursSlug,
    CoOccursToken,
    Contradicts,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::References => "references",
            Self::ContainedIn => "contained_in",
            Self::SupersededBy => "superseded_by",
            Self::DerivedFrom => "derived_from",
            Self::CoOccursSlug => "co_occurs_slug",
            Self::CoOccursToken => "co_occurs_token",
            Self::Contradicts => "contradicts",
        }
    }

    pub fn parse(s: &str) -> Result<Self, MemoryGraphError> {
        match s {
            "references" => Ok(Self::References),
            "contained_in" => Ok(Self::ContainedIn),
            "superseded_by" => Ok(Self::SupersededBy),
            "derived_from" => Ok(Self::DerivedFrom),
            "co_occurs_slug" => Ok(Self::CoOccursSlug),
            "co_occurs_token" => Ok(Self::CoOccursToken),
            "contradicts" => Ok(Self::Contradicts),
            other => Err(MemoryGraphError::InvalidEdgeKind(other.into())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    Frontmatter,
    InlineLink,
    Index,
    Synthesized,
    UserStated,
}

impl Provenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Frontmatter => "frontmatter",
            Self::InlineLink => "inline-link",
            Self::Index => "index",
            Self::Synthesized => "synthesized",
            Self::UserStated => "user-stated",
        }
    }

    pub fn parse(s: &str) -> Result<Self, MemoryGraphError> {
        match s {
            "frontmatter" => Ok(Self::Frontmatter),
            "inline-link" => Ok(Self::InlineLink),
            "index" => Ok(Self::Index),
            "synthesized" => Ok(Self::Synthesized),
            "user-stated" => Ok(Self::UserStated),
            other => Err(MemoryGraphError::InvalidProvenance(other.into())),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryNode {
    pub id: String,
    pub kind: MemoryKind,
    pub name: String,
    pub description: Option<String>,
    pub file_path: String,
    pub body_preview: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub ttl_days: Option<u32>,
    pub content_hash: String,
    pub indexed_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryEdge {
    pub id: Option<i64>,
    pub src_id: String,
    pub dst_id: String,
    pub kind: EdgeKind,
    pub line: Option<u32>,
    pub metadata: Option<serde_json::Value>,
    pub provenance: Provenance,
    pub confidence: f32,
    pub created_at: i64,
}
