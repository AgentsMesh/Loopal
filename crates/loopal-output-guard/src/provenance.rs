use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryContentKind {
    Binary,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryProvenance {
    Known(Vec<u8>),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown-provenance {kind} content is denied")]
pub struct BinaryProvenanceError {
    pub kind: BinaryContentKind,
}

impl std::fmt::Display for BinaryContentKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Binary => "binary",
            Self::Image => "image",
        })
    }
}

pub fn require_known_binary_provenance(
    kind: BinaryContentKind,
    provenance: BinaryProvenance,
) -> Result<Vec<u8>, BinaryProvenanceError> {
    match provenance {
        BinaryProvenance::Known(value) => Ok(value),
        BinaryProvenance::Unknown => Err(BinaryProvenanceError { kind }),
    }
}
