//! Optimized glob and grep search with parallel traversal.

mod binary;
mod glob;
mod grep;
mod grep_match;
mod walker;

pub use glob::glob_search;
pub use grep::grep_search;
