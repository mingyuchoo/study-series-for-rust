use chrono::{DateTime,
             Utc};
use gm_application::Clock;

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> { Utc::now() }
}
