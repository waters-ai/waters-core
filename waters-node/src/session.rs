use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub node_id: String,
    pub mission_id: String,
    pub started_at: String,
    pub updated_at: String,
    pub turn_count: u64,
    pub system_prompt: String,
    pub history: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub session: Session,
    pub node_state: serde_json::Value,
    pub saved_at: String,
}

pub struct SessionManager {
    session_dir: PathBuf,
    checkpoint_path: PathBuf,
    current: Option<Session>,
}

impl SessionManager {
    pub fn new(session_dir: &Path) -> Self {
        std::fs::create_dir_all(session_dir).ok();
        let checkpoint_path = session_dir.join("..").join("checkpoints");
        std::fs::create_dir_all(&checkpoint_path).ok();
        SessionManager {
            session_dir: session_dir.to_path_buf(),
            checkpoint_path,
            current: None,
        }
    }

    pub fn start(&mut self, node_id: &str, mission_id: &str, system_prompt: &str) -> String {
        let now = chrono::Utc::now().to_rfc3339();
        let session = Session {
            session_id: uuid::Uuid::new_v4().to_string(),
            node_id: node_id.to_string(),
            mission_id: mission_id.to_string(),
            started_at: now.clone(),
            updated_at: now,
            turn_count: 0,
            system_prompt: system_prompt.to_string(),
            history: Vec::new(),
        };
        let id = session.session_id.clone();
        self.current = Some(session);
        info!("Session started: {}", id);
        id
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        if let Some(ref mut session) = self.current {
            session.history.push(Message {
                role: role.to_string(),
                content: content.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
            session.turn_count += 1;
            session.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(ref session) = self.current {
            let path = self.session_dir.join(format!("{}.json", session.session_id));
            let json = serde_json::to_string_pretty(session)?;
            std::fs::write(&path, json)?;
            info!("Session saved: {} ({} turns)", session.session_id, session.turn_count);
        }
        Ok(())
    }

    /// Checkpoint перед каждым шагом — полный снэпшот
    pub fn save_checkpoint(&self, node_state: &serde_json::Value) -> anyhow::Result<()> {
        if let Some(ref session) = self.current {
            let checkpoint = Checkpoint {
                session: session.clone(),
                node_state: node_state.clone(),
                saved_at: chrono::Utc::now().to_rfc3339(),
            };
            let path = self.checkpoint_path.join("latest.json");
            let json = serde_json::to_string_pretty(&checkpoint)?;
            std::fs::write(&path, json)?;
            info!("Checkpoint saved: turn {}", session.turn_count);
        }
        Ok(())
    }

    /// Восстановление после падения — читает последний чекпоинт
    pub fn resume_from_checkpoint() -> anyhow::Result<Option<Checkpoint>> {
        let path = PathBuf::from(".waters/checkpoints/latest.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let checkpoint: Checkpoint = serde_json::from_str(&content)?;
            info!("Resumed from checkpoint: turn {}", checkpoint.session.turn_count);
            Ok(Some(checkpoint))
        } else {
            Ok(None)
        }
    }

    /// Очистить чекпоинт после успешного шага
    pub fn clear_checkpoint() -> anyhow::Result<()> {
        let path = PathBuf::from(".waters/checkpoints/latest.json");
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Восстановить сессию из чекпоинта (после краша)
    pub fn restore_from(&mut self, session: Session) {
        info!("Session restored from checkpoint: {} (turn {})", session.session_id, session.turn_count);
        self.current = Some(session);
    }

    pub fn resume(&mut self, session_id: &str) -> anyhow::Result<bool> {
        let path = self.session_dir.join(format!("{}.json", session_id));
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let session: Session = serde_json::from_str(&content)?;
            info!("Session resumed: {} ({} turns)", session.session_id, session.turn_count);
            self.current = Some(session);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn current(&self) -> Option<&Session> {
        self.current.as_ref()
    }

    pub fn history_context(&self, max_turns: usize) -> String {
        self.current.as_ref().map(|s| {
            let recent: Vec<String> = s.history.iter().rev()
                .take(max_turns * 2).rev()
                .map(|m| {
                    if m.role == "user" {
                        format!("User: {}", m.content)
                    } else {
                        format!("Assistant: {}", m.content)
                    }
                })
                .collect();
            recent.join("\n")
        }).unwrap_or_default()
    }

    pub fn list_sessions(&self) -> Vec<String> {
        let mut sessions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.session_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(s) = serde_json::from_str::<Session>(&content) {
                            sessions.push(format!("{} | {} turns | {}",
                                &s.session_id[..8], s.turn_count, s.updated_at));
                        }
                    }
                }
            }
        }
        sessions.sort();
        sessions.reverse();
        sessions
    }
}
