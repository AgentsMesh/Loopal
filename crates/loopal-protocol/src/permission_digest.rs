use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

const PREFIX: &str = "sha256:";
const ENCODED_LEN: usize = PREFIX.len() + 64;

macro_rules! digest_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(PREFIX)?;
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl FromStr for $name {
            type Err = DigestParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                parse(value).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                value.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

digest_type!(PermissionActionDigest);
digest_type!(PermissionDisplayDigest);
digest_type!(PermissionSchemaDigest);
digest_type!(PermissionIntentDigest);
digest_type!(WorkflowAttemptCapabilityDigest);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DigestParseError;

impl fmt::Display for DigestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected sha256: followed by 64 lowercase hexadecimal digits")
    }
}

impl std::error::Error for DigestParseError {}

fn parse(value: &str) -> Result<[u8; 32], DigestParseError> {
    if value.len() != ENCODED_LEN || !value.starts_with(PREFIX) {
        return Err(DigestParseError);
    }
    let encoded = &value.as_bytes()[PREFIX.len()..];
    let mut bytes = [0; 32];
    for (index, pair) in encoded.chunks_exact(2).enumerate() {
        bytes[index] = nibble(pair[0])?
            .checked_mul(16)
            .and_then(|high| high.checked_add(nibble(pair[1]).ok()?))
            .ok_or(DigestParseError)?;
    }
    Ok(bytes)
}

fn nibble(value: u8) -> Result<u8, DigestParseError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(DigestParseError),
    }
}

pub(crate) fn framed_sha256(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_be_bytes());
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}
