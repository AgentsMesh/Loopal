#![allow(dead_code, unused_variables)]

//! Deterministic boundary harness for the production root-start typestate.

extern crate self as anyhow;
extern crate self as loopal_agent_client;
extern crate self as loopal_agent_hub;
extern crate self as loopal_protocol;
extern crate self as tracing;
extern crate self as uuid;

use std::fmt;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

const FAIL_BIND: u8 = 1 << 0;
const FAIL_RECOVERY: u8 = 1 << 1;
const FAIL_START: u8 = 1 << 2;
const MISMATCH_SESSION: u8 = 1 << 3;
const FAIL_ACTIVATION: u8 = 1 << 4;
const FAIL_RUNTIME_SHUTDOWN: u8 = 1 << 5;
const FAIL_PROCESS_SHUTDOWN: u8 = 1 << 6;

static FAILURE_MASK: AtomicU8 = AtomicU8::new(0);
static RECOVERIES: AtomicUsize = AtomicUsize::new(0);
static ACTIVATIONS: AtomicUsize = AtomicUsize::new(0);
static RUNTIME_SHUTDOWNS: AtomicUsize = AtomicUsize::new(0);
static PROCESS_SHUTDOWNS: AtomicUsize = AtomicUsize::new(0);
static WARNINGS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_export]
macro_rules! bail {
    ($format:literal $(, $argument:expr)* $(,)?) => {
        return Err($crate::Error(format!($format $(, $argument)*)))
    };
    ($error:expr $(,)?) => {
        return Err($crate::Error($error.to_string()))
    };
}

#[macro_export]
macro_rules! warn {
    ($($argument:tt)*) => {{
        let _ = stringify!($($argument)*);
        $crate::WARNINGS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }};
}

fn fails(flag: u8) -> bool {
    FAILURE_MASK.load(Ordering::SeqCst) & flag != 0
}

#[path = "bootstrap_start_root_boundary.rs"]
mod boundary;

pub use boundary::{
    AgentClient, AgentProcess, ClientConnection, Hub, QualifiedAddress, ROOT_AGENT_NAME,
    StartAgentParams, Uuid, agent_io, states, workflow,
};

#[path = "../../src/bootstrap/hub/typestate/start_root.rs"]
mod start_root;

#[cfg(test)]
#[path = "bootstrap_start_root_tests.rs"]
mod tests;
