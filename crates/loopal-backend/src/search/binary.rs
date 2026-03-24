//! Fast binary file detection via NUL-byte sampling.

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Size of the head buffer sampled for NUL bytes.
const SAMPLE_SIZE: usize = 8192;

/// Returns `true` if the file likely contains binary content.
///
/// Reads the first 8 KB and checks for NUL bytes — a reliable heuristic
/// used by Git and ripgrep.  Returns `true` on read failure (skip).
pub fn is_likely_binary(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return true;
    };
    let mut buf = [0u8; SAMPLE_SIZE];
    let Ok(n) = file.read(&mut buf) else {
        return true;
    };
    buf[..n].contains(&0)
}
