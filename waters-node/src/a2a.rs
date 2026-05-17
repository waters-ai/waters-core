/// A2A (Agent-to-Agent) — Google протокол для меж-агентского общения
/// Позволяет waters-node говорить с любыми A2A-совместимыми агентами

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

/// A2A Task status (по спецификации Google A2A)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum A2aTaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Failed,
    Canceled,
}

/// A2A Message (ядро протокола)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aMessage {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

impl A2aMessage {
    pub fn new(method: &str, params: serde_json::Value) -> Self {
        A2aMessage {
            jsonrpc: "2.0".into(),
            id: uuid::Uuid::new_v4().to_string(),
            method: method.to_string(),
            params,
        }
    }

    /// Создать A2A-запрос из нашей @agent команды
    pub fn from_agent_command(target: &str, action: &str, payload: serde_json::Value) -> Self {
        A2aMessage::new("tasks/send", serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "sessionId": uuid::Uuid::new_v4().to_string(),
            "message": {
                "jsonrpc": "2.0",
                "method": "message/send",
                "params": {
                    "target": target,
                    "action": action,
                    "payload": payload,
                }
            },
        }))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// Зарегистрированный внешний A2A-агент
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aPeer {
    pub name: String,
    pub url: String,
    pub agent_id: String,
    pub provider: String, // "hermes", "google", "crewai", "unknown"
    pub capabilities: Vec<String>,
    pub last_seen: String,
    pub status: String,
}

impl A2aPeer {
    pub fn new(name: &str, url: &str, provider: &str) -> Self {
        A2aPeer {
            name: name.to_string(),
            url: url.to_string(),
            agent_id: format!("a2a-{}", name),
            provider: provider.to_string(),
            capabilities: vec![],
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "online".into(),
        }
    }
}

/// A2A-адаптер — преобразует наш @agent протокол в A2A
pub struct A2aAdapter {
    peers: Vec<A2aPeer>,
    local_agent_id: String,
}

impl A2aAdapter {
    pub fn new(agent_id: &str) -> Self {
        A2aAdapter {
            peers: Vec::new(),
            local_agent_id: format!("a2a-{}", agent_id),
        }
    }

    /// Отправить A2A-запрос внешнему агенту
    pub async fn send(&self, target: &str, action: &str, payload: serde_json::Value) -> Result<String, String> {
        let peer = self.peers.iter().find(|p| p.name == target || p.agent_id == target)
            .ok_or_else(|| format!("A2A agent '{}' not found", target))?;

        let msg = A2aMessage::from_agent_command(target, action, payload);
        let url = format!("{}/message:send", peer.url.trim_end_matches('/'));

        let client = reqwest::Client::new();
        match client.post(&url)
            .header("Content-Type", "application/json")
            .json(&msg)
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(text) = resp.text().await {
                    info!("A2A: sent to {} → {}", target, &text[..80.min(text.len())]);
                    Ok(text)
                } else {
                    Err("Empty response from A2A agent".into())
                }
            }
            Err(e) => Err(format!("A2A connection failed: {}", e)),
        }
    }

    /// Зарегистрировать внешнего A2A-агента
    pub fn register(&mut self, name: &str, url: &str, provider: &str) {
        // Удаляем дубликат если был
        self.peers.retain(|p| p.name != name && p.url != url);
        let peer = A2aPeer::new(name, url, provider);
        info!("A2A: registered peer '{}' ({}) from {}", name, url, provider);
        self.peers.push(peer);
    }

    pub fn list(&self) -> &[A2aPeer] { &self.peers }

    /// mDNS-поиск A2A-агентов в локальной сети
    pub async fn discover(&mut self) -> Vec<A2aPeer> {
        // Пробуем найти A2A-агентов через mDNS-запрос _a2a._tcp
        let mut discovered = Vec::new();
        // Заглушка — будет заменена на реальный mDNS
        info!("A2A: discover started (mDNS _a2a._tcp)");
        discovered
    }

    /// Наш A2A-endpoint для внешних запросов
    pub fn local_endpoint(&self) -> String {
        format!("/a2a/v1/{}", self.local_agent_id)
    }

    pub fn summary(&self) -> String {
        let mut out = format!("🔄 A2A Gateway (протокол Google Agent2Agent)\n");
        out.push_str(&format!("  Local agent: {}\n\n", self.local_agent_id));
        if self.peers.is_empty() {
            out.push_str("  Нет подключённых A2A-агентов.\n");
            out.push_str("  Добавить: /a2a connect <url> [provider]\n");
            out.push_str("  Искать: /a2a discover\n");
        } else {
            out.push_str("  Подключённые A2A-агенты:\n");
            for p in &self.peers {
                out.push_str(&format!("    {} — {} ({}) — {}\n", p.name, p.provider, p.url, p.status));
            }
        }
        out
    }
}
