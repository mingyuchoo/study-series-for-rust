//! Small application ports for nondeterministic values.

use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub trait IdGenerator: Send + Sync {
    fn next(&self, prefix: &str) -> String;
}

#[derive(Default)]
pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn next(&self, prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }
}
