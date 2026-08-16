pub mod audit;
pub mod guard;
pub mod hooks;
pub mod merged;
pub mod redactor;
pub mod resolver;
pub mod template;

pub use audit::{JsonlAuditSink, RuntimeOp, default_telemetry_dir};
pub use guard::{SECRET_REJECTION_MESSAGE, WIRE_REF_MARKER, input_contains_secret_ref};
pub use hooks::{
    apply_redactor, apply_redactor_with_audit, apply_resolver, apply_resolver_with_audit,
    detect_argv_exposure, record_redaction_hits, record_redaction_hits_with_audit,
};
pub use merged::MergedVault;
pub use redactor::Redactor;
pub use resolver::{ResolverReport, collect_wire_refs, resolve_in_value};
pub use template::{
    TranslationStats, TranslationView, collect_author_names, collect_wire_names,
    expand_to_plaintext, translate_outbound,
};
