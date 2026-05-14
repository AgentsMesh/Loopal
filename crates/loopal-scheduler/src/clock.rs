use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct ManualClock {
    now: std::sync::Mutex<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(initial: DateTime<Utc>) -> Self {
        Self {
            now: std::sync::Mutex::new(initial),
        }
    }

    pub fn set(&self, time: DateTime<Utc>) {
        *self.now.lock().unwrap() = time;
    }

    pub fn advance(&self, duration: chrono::Duration) {
        let mut now = self.now.lock().unwrap();
        *now += duration;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}
