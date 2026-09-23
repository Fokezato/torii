use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub struct ActivityLog {
    entries: Mutex<VecDeque<LogEntry>>,
    next_id: Mutex<u64>,
}

impl ActivityLog {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(MAX_ENTRIES)),
            next_id: Mutex::new(1),
        }
    }

    fn push(&self, level: &str, message: String) {
        let id = {
            let mut next_id = self.next_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };
        let entry = LogEntry {
            id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            message,
        };
        let mut entries = self.entries.lock().unwrap();
        entries.push_front(entry);
        while entries.len() > MAX_ENTRIES {
            entries.pop_back();
        }
    }

    pub fn info(&self, message: impl Into<String>) {
        self.push("info", message.into());
    }

    pub fn error(&self, message: impl Into<String>) {
        self.push("error", message.into());
    }

    pub fn list(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }
}
