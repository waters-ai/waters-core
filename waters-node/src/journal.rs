use std::path::{Path, PathBuf};
use std::io::Write;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::store::KvStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub timestamp: String,
    pub agent_id: String,
    pub event: String,
    pub detail: String,
}

pub struct AgentJournal {
    log_dir: PathBuf,
    kvstore: Option<Arc<KvStore>>,
}

impl AgentJournal {
    pub fn new(log_dir: &Path, kvstore: Option<Arc<KvStore>>) -> Self {
        std::fs::create_dir_all(log_dir).ok();
        AgentJournal { log_dir: log_dir.to_path_buf(), kvstore }
    }

    pub fn log(&self, agent_id: &str, event: &str, detail: &str) {
        let entry = JournalEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            agent_id: agent_id.to_string(),
            event: event.to_string(),
            detail: detail.to_string(),
        };

        if let Ok(line) = serde_json::to_string(&entry) {
            // Write to file always
            let path = self.log_dir.join(format!("{}.log", agent_id));
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true).append(true).open(&path)
            {
                let _ = writeln!(file, "{}", line);
            }

            // Write to KvStore if available
            if let Some(ref kv) = self.kvstore {
                let key = format!("journal:{}", agent_id);
                kv.list_append(&key, &line, 1000).ok();
                kv.publish(&format!("events:{}", agent_id), &line).ok();
            }

            info!("[{}] {}: {}", agent_id, event, &detail[..detail.len().min(80)]);
        }
    }

    pub fn read(&self, agent_id: &str, count: usize) -> Vec<JournalEntry> {
        // Try KvStore first (has more recent data), fallback to file
        if let Some(ref kv) = self.kvstore {
            let key = format!("journal:{}", agent_id);
            if let Ok(items) = kv.list_range(&key, 0, count as isize - 1) {
                let entries: Vec<JournalEntry> = items.iter()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                if !entries.is_empty() { return entries; }
            }
        }

        let path = self.log_dir.join(format!("{}.log", agent_id));
        if !path.exists() { return vec![]; }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        content.lines()
            .filter_map(|l| serde_json::from_str::<JournalEntry>(l).ok())
            .rev().take(count).collect()
    }

    pub fn list_agents(&self) -> Vec<String> {
        if let Some(ref kv) = self.kvstore {
            if let Ok(keys) = kv.list_keys("journal:") {
                let agents: Vec<String> = keys.iter()
                    .filter_map(|k| k.strip_prefix("journal:").map(String::from))
                    .collect();
                if !agents.is_empty() { return agents; }
            }
        }
        let mut agents = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    if path.extension().map(|e| e == "log").unwrap_or(false) {
                        agents.push(name.to_string());
                    }
                }
            }
        }
        agents.sort();
        agents
    }
}
