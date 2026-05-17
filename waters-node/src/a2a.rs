/// A2A (Agent-to-Agent) — Google протокол для меж-агентского общения
/// Позволяет waters-node говорить с любыми A2A-совместимыми агентами

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// Токен для входящих A2A-запросов (из WATERS_A2A_TOKEN)
    auth_token: String,
    /// Белый список — только эти A2A-агенты могут слать запросы
    allowed_peers: HashSet<String>,
    /// Rate limiter — макс запросов в минуту
    rate_limit: AtomicU64,
    rate_window: AtomicU64,
}

impl A2aAdapter {
    pub fn new(agent_id: &str) -> Self {
        let token = std::env::var("WATERS_A2A_TOKEN").unwrap_or_default();
        A2aAdapter {
            peers: Vec::new(),
            local_agent_id: format!("a2a-{}", agent_id),
            auth_token: token,
            allowed_peers: HashSet::new(),
            rate_limit: AtomicU64::new(60),
            rate_window: AtomicU64::new(60),
        }
    }

    /// Проверить авторизацию входящего A2A-запроса
    pub fn check_auth(&self, token: &str) -> bool {
        if self.auth_token.is_empty() {
            return true; // если токен не задан — пропускаем все (для отладки)
        }
        token == self.auth_token
    }

    /// Проверить, разрешён ли этот A2A-пир
    pub fn is_peer_allowed(&self, peer_id: &str) -> bool {
        if self.allowed_peers.is_empty() {
            return true; // если белый список пуст — пропускаем всех
        }
        self.allowed_peers.contains(peer_id)
    }

    /// Проверить rate limit
    pub fn check_rate_limit(&self) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let window = self.rate_window.load(Ordering::Relaxed);
        if now > window {
            self.rate_window.store(now + 60, Ordering::Relaxed);
            self.rate_limit.store(0, Ordering::Relaxed);
        }
        let count = self.rate_limit.fetch_add(1, Ordering::Relaxed);
        count < 120 // макс 120 запросов в минуту
    }

    /// Разрешить A2A-пиру доступ
    pub fn allow_peer(&mut self, peer_id: &str) {
        self.allowed_peers.insert(peer_id.to_string());
        info!("A2A Security: allowed peer '{}'", peer_id);
    }

    /// Заблокировать A2A-пира
    pub fn block_peer(&mut self, peer_id: &str) {
        self.allowed_peers.remove(peer_id);
        info!("A2A Security: blocked peer '{}'", peer_id);
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

    /// Обработать входящий A2A-запрос от внешнего агента
    /// Возвращает JSON-ответ и HTTP-статус (200/401/429)
    pub fn handle_request(&self, body: &str, auth_token: &str) -> (String, u16) {
        // 1. Проверка аутентификации
        if !self.check_auth(auth_token) {
            warn!("A2A Security: UNAUTHORIZED access attempt");
            let error = serde_json::json!({
                "jsonrpc": "2.0", "id": null,
                "error": {"code": -32001, "message": "Unauthorized"}
            });
            return (serde_json::to_string(&error).unwrap_or_default(), 401);
        }
        // 2. Rate limit
        if !self.check_rate_limit() {
            warn!("A2A Security: rate limit exceeded");
            let error = serde_json::json!({
                "jsonrpc": "2.0", "id": null,
                "error": {"code": -32002, "message": "Too Many Requests"}
            });
            return (serde_json::to_string(&error).unwrap_or_default(), 429);
        }
        // 3. Парсинг и обработка
        match A2aMessage::from_json(body) {
            Some(msg) => {
                // 4. Проверка ACL пира
                let peer_id = msg.params.get("target").and_then(|v| v.as_str()).unwrap_or("unknown");
                if !self.is_peer_allowed(peer_id) {
                    warn!("A2A Security: peer '{}' not allowed", peer_id);
                    let error = serde_json::json!({
                        "jsonrpc": "2.0", "id": &msg.id,
                        "error": {"code": -32003, "message": "Access denied"}
                    });
                    return (serde_json::to_string(&error).unwrap_or_default(), 403);
                }
                info!("A2A: incoming {} from ({})", msg.method, &msg.id[..8]);
                let result = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": msg.id,
                    "result": {
                        "id": uuid::Uuid::new_v4().to_string(),
                        "status": "working",
                        "message": format!("A2A request '{}' received by waters-node", msg.method),
                    }
                });
                (serde_json::to_string(&result).unwrap_or_default(), 200)
            }
            None => {
                let error = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": "Parse error"}
                });
                (serde_json::to_string(&error).unwrap_or_default(), 400)
            }
        }
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
