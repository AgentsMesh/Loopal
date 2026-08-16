pub mod audit;
pub mod error;
pub mod vault;

pub use audit::{
    AuditError, AuditMetadata, AuditResult, AuditSink, NoopAuditSink, ProtectedOp, VaultOp,
};
pub use error::{VaultError, VaultResult};
pub use secrecy::SecretString;
pub use vault::Vault;
