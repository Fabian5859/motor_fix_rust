use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct IdGenerator {
    counter: AtomicU64,
}

impl IdGenerator {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }

    /// Genera un ClOrdID único: ID-YYYYMMDD-HHMMSS-COUNT
    pub fn next_id(&self) -> String {
        let now = Utc::now();
        let count = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("ID-{}-{:04}", now.format("%Y%m%d-%H%M%S"), count)
    }
}
