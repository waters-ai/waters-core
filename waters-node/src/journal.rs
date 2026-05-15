use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::io::Write;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub timestamp: String,
    pub agent_id: String,
    pub event: String,
    pub detail: String,
}

pub struct AgentJournal {
    log_dir: PathBuf,
}

impl AgentJournal {
    pub fn new(log_dir: &Path) -> Self {
        std::fs::create_dir_all(log_dir).ok();
        AgentJournal {
            log_dir: log_dir.to_path_buf(),
        }
    }

    pub fn log(&self, agent_id: &str, event: &str, detail: &str) {
        let entry = JournalEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            agent_id: agent_id.to_string(),
            event: event.to_string(),
            detail: detail.to_string(),
        };

        if let Ok(line) = serde_json::to_string(&entry) {
            let path = self.log_dir.join(format!("{}.log", agent_id));
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = writeln!(file, "{}", line);
            }
            info!("[{}] {}: {}", agent_id, event, &detail[..detail.len().min(80)]);
        }
    }

    pub fn read(&self, agent_id: &str, count: usize) -> Vec<JournalEntry> {
        let path = self.log_dir.join(format!("{}.log", agent_id));
        if !path.exists() { return vec![]; }

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        content.lines()
            .filter_map(|l| serde_json::from_str::<JournalEntry>(l).ok())
            .rev()
            .take(count)
            .collect()
    }

    pub fn list_agents(&self) -> Vec<String> {
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
