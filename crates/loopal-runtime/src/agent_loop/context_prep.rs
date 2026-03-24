//! Ephemeral context preparation — now delegated to ContextStore.
//!
//! Previously this module performed clone + strip + truncate + safety net.
//! With the ContextStore architecture, sync degradation runs persistently on
//! every push, so this module is now a thin delegation to `store.prepare_for_llm()`.
//!
//! Kept as a module for backward compatibility with turn_exec.rs's method call.

// This module is intentionally minimal. The prepare_for_llm logic now lives
// in ContextStore::prepare_for_llm() (crates/loopal-context/src/store.rs).
// No impl methods needed here — turn_exec.rs calls store.prepare_for_llm() directly.
