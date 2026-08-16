use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

#[derive(Default)]
pub struct HarnessVault {
    pub(super) entries: Mutex<HashMap<String, String>>,
    pub(super) failing: AtomicBool,
    pub(super) gets: Mutex<Vec<Value>>,
}

impl HarnessVault {
    pub fn insert(&self, name: &str, plaintext: &str) {
        self.entries
            .lock()
            .unwrap()
            .insert(name.to_string(), plaintext.to_string());
    }

    pub fn set_failing(&self, failing: bool) {
        self.failing.store(failing, Ordering::Release);
    }

    pub fn gets(&self) -> Vec<Value> {
        self.gets.lock().unwrap().clone()
    }
}

pub struct PermissionDesk {
    pub(super) allow: AtomicBool,
    pub(super) hold: AtomicBool,
    pub(super) asks: Mutex<Vec<Value>>,
    pub(super) question_answers: Mutex<Vec<String>>,
    pub(super) question_asks: Mutex<Vec<Value>>,
    pub(super) plan_decision: Mutex<String>,
    pub(super) plan_requests: Mutex<Vec<Value>>,
}

impl Default for PermissionDesk {
    fn default() -> Self {
        Self {
            allow: Default::default(),
            hold: Default::default(),
            asks: Default::default(),
            question_answers: Default::default(),
            question_asks: Default::default(),
            plan_decision: Mutex::new("approve".into()),
            plan_requests: Default::default(),
        }
    }
}

impl PermissionDesk {
    pub fn set_allow(&self, allow: bool) {
        self.allow.store(allow, Ordering::Release);
    }

    pub fn set_hold(&self, hold: bool) {
        self.hold.store(hold, Ordering::Release);
    }

    pub fn asks(&self) -> Vec<Value> {
        self.asks.lock().unwrap().clone()
    }

    pub fn set_question_answers(&self, answers: &[&str]) {
        *self.question_answers.lock().unwrap() = answers.iter().map(|s| s.to_string()).collect();
    }

    pub fn question_asks(&self) -> Vec<Value> {
        self.question_asks.lock().unwrap().clone()
    }

    pub fn set_plan_decision(&self, decision: &str) {
        *self.plan_decision.lock().unwrap() = decision.to_string();
    }

    pub fn plan_requests(&self) -> Vec<Value> {
        self.plan_requests.lock().unwrap().clone()
    }
}
