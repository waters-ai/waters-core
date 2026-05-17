use crate::group_chat::GroupChat;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════
// Agent-to-Agent ACL — кто кому может писать
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAcl {
    /// Разрешённые пары from→to
    allowed: HashMap<String, HashSet<String>>,
    /// Запрещённые пары from→to
    blocked: HashMap<String, HashSet<String>>,
    /// По умолчанию: разрешено (true) или запрещено (false)
    default_allow: bool,
    path: PathBuf,
}

impl AgentAcl {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("agent_acl.json");
        let (allowed, blocked, default_allow) = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        let a = serde_json::from_value(data["allowed"].clone()).unwrap_or_default();
                        let b = serde_json::from_value(data["blocked"].clone()).unwrap_or_default();
                        let def = data["default_allow"].as_bool().unwrap_or(true);
                        (a, b, def)
                    } else {
                        (HashMap::new(), HashMap::new(), true)
                    }
                }
                Err(_) => (HashMap::new(), HashMap::new(), true),
            }
        } else {
            (HashMap::new(), HashMap::new(), true)
        };

        AgentAcl {
            allowed,
            blocked,
            default_allow,
            path,
        }
    }

    /// Разрешить from → to
    pub fn allow(&mut self, from: &str, to: &str) {
        self.allowed
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
        self.blocked.entry(from.to_string()).or_default().remove(to);
        self.save();
        info!("AgentACL: allowed {} → {}", from, to);
    }

    /// Запретить from → to
    pub fn block(&mut self, from: &str, to: &str) {
        self.blocked
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
        self.allowed.entry(from.to_string()).or_default().remove(to);
        self.save();
        info!("AgentACL: blocked {} → {}", from, to);
    }

    /// Запретить from → все (*)
    pub fn block_all(&mut self, from: &str) {
        self.blocked
            .entry(from.to_string())
            .or_default()
            .insert("*".to_string());
        self.save();
        warn!("AgentACL: blocked {} → * (all agents)", from);
    }

    /// Проверить, может ли from отправить to
    pub fn can_send(&self, from: &str, to: &str) -> bool {
        // Явный запрет from→to
        if let Some(blocked) = self.blocked.get(from) {
            if blocked.contains(to) || blocked.contains("*") {
                return false;
            }
        }
        // Явное разрешение from→to
        if let Some(allowed) = self.allowed.get(from) {
            if allowed.contains(to) || allowed.contains("*") {
                return true;
            }
        }
        self.default_allow
    }

    /// Удалить все правила для from
    pub fn reset(&mut self, from: &str) {
        self.allowed.remove(from);
        self.blocked.remove(from);
        self.save();
        info!("AgentACL: reset rules for {}", from);
    }

    pub fn summary(&self) -> String {
        let mut out = format!(
            "🔒 Agent ACL (default: {})",
            if self.default_allow {
                "✅ разрешено"
            } else {
                "❌ запрещено"
            }
        );
        for (from, targets) in &self.allowed {
            out.push_str(&format!(
                "\n  ✅ {} → [{}]",
                from,
                targets.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        for (from, targets) in &self.blocked {
            out.push_str(&format!(
                "\n  ❌ {} → [{}]",
                from,
                targets.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        out
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let data = serde_json::json!({
            "allowed": self.allowed,
            "blocked": self.blocked,
            "default_allow": self.default_allow,
        });
        let _ = fs::write(
            &self.path,
            serde_json::to_string_pretty(&data).unwrap_or_default(),
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub msg_id: String,
    pub msg_type: AgentMsgType,
    pub from: String,
    pub from_role: String,
    pub to: String,
    pub channel: String,
    pub payload: serde_json::Value,
    pub reply_to: Option<String>,
    pub ttl_secs: u32,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentMsgType {
    /// Запрос к другому агенту
    Request { action: String },
    /// Ответ на запрос
    Response {
        status: String,
        data: serde_json::Value,
    },
    /// Передача данных без ожидания ответа
    Broadcast { topic: String },
    /// Вызов инструмента другого агента
    ToolCall {
        tool: String,
        args: serde_json::Value,
    },
    /// Результат вызова инструмента
    ToolResult {
        tool: String,
        result: serde_json::Value,
        error: Option<String>,
    },
    /// Координация — предложение, согласие, отказ
    Coordinate { proposal: String, decision: String },
    /// Findings — поделиться результатом
    Finding {
        finding_type: String,
        confidence: f64,
    },
}

impl AgentMessage {
    pub fn new_request(
        from: &str,
        to: &str,
        channel: &str,
        action: &str,
        payload: serde_json::Value,
    ) -> Self {
        AgentMessage {
            msg_id: uuid::Uuid::new_v4().to_string(),
            msg_type: AgentMsgType::Request {
                action: action.to_string(),
            },
            from: from.to_string(),
            from_role: "agent".into(),
            to: to.to_string(),
            channel: channel.to_string(),
            payload,
            reply_to: None,
            ttl_secs: 60,
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn new_response(
        msg_id: &str,
        from: &str,
        to: &str,
        channel: &str,
        status: &str,
        data: serde_json::Value,
    ) -> Self {
        AgentMessage {
            msg_id: uuid::Uuid::new_v4().to_string(),
            msg_type: AgentMsgType::Response {
                status: status.to_string(),
                data,
            },
            from: from.to_string(),
            from_role: "agent".into(),
            to: to.to_string(),
            channel: channel.to_string(),
            payload: serde_json::json!({}),
            reply_to: Some(msg_id.to_string()),
            ttl_secs: 60,
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn new_broadcast(
        from: &str,
        channel: &str,
        topic: &str,
        payload: serde_json::Value,
    ) -> Self {
        AgentMessage {
            msg_id: uuid::Uuid::new_v4().to_string(),
            msg_type: AgentMsgType::Broadcast {
                topic: topic.to_string(),
            },
            from: from.to_string(),
            from_role: "agent".into(),
            to: "*".into(),
            channel: channel.to_string(),
            payload,
            reply_to: None,
            ttl_secs: 300,
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn new_tool_call(
        from: &str,
        to: &str,
        channel: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> Self {
        AgentMessage {
            msg_id: uuid::Uuid::new_v4().to_string(),
            msg_type: AgentMsgType::ToolCall {
                tool: tool.to_string(),
                args,
            },
            from: from.to_string(),
            from_role: "agent".into(),
            to: to.to_string(),
            channel: channel.to_string(),
            payload: serde_json::json!({}),
            reply_to: None,
            ttl_secs: 120,
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn new_coordinate(
        from: &str,
        to: &str,
        channel: &str,
        proposal: &str,
        decision: &str,
    ) -> Self {
        AgentMessage {
            msg_id: uuid::Uuid::new_v4().to_string(),
            msg_type: AgentMsgType::Coordinate {
                proposal: proposal.to_string(),
                decision: decision.to_string(),
            },
            from: from.to_string(),
            from_role: "agent".into(),
            to: to.to_string(),
            channel: channel.to_string(),
            payload: serde_json::json!({}),
            reply_to: None,
            ttl_secs: 30,
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

pub struct AgentChat {
    kvstore: std::sync::Arc<crate::store::KvStore>,
    acl: AgentAcl,
}

impl AgentChat {
    pub fn new(kvstore: std::sync::Arc<crate::store::KvStore>) -> Self {
        let acl = AgentAcl::new(&PathBuf::from(".waters"));
        AgentChat { kvstore, acl }
    }

    pub fn acl(&self) -> &AgentAcl {
        &self.acl
    }
    pub fn acl_mut(&mut self) -> &mut AgentAcl {
        &mut self.acl
    }

    /// Отправить сообщение агенту — с проверкой ACL
    pub fn send(&self, msg: &AgentMessage, group_id: u8) -> Result<(), Box<dyn std::error::Error>> {
        if !self.acl.can_send(&msg.from, &msg.to) {
            warn!(
                "AgentACL: BLOCKED {} → {} (no permission)",
                msg.from, msg.to
            );
            return Err("Agent communication blocked by owner ACL".into());
        }
        let channel_key = format!("agent:{}:{}:{}", msg.channel, msg.from, msg.to);
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);
        let json = msg.to_json();
        let _ = kv.xadd(&channel_key, &[("msg", &json)], 1000);
        info!(
            "AgentChat: {} → {} [{}] {:?}",
            &msg.from[..8.min(msg.from.len())],
            &msg.to[..8.min(msg.to.len())],
            msg.channel,
            std::mem::discriminant(&msg.msg_type)
        );
        Ok(())
    }

    /// Прочитать входящие сообщения для агента (из Redis Stream)
    pub fn read(
        &self,
        agent_id: &str,
        channel: &str,
        group_id: u8,
        _count: u32,
    ) -> Vec<AgentMessage> {
        let direct_key = format!("agent:{}:{}:{}", channel, "*", agent_id);
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);
        let mut messages = Vec::new();
        if let Ok(Some(data)) = kv.get(&direct_key) {
            if let Some(msg) = AgentMessage::from_json(&data) {
                messages.push(msg);
            }
        }
        messages
    }

    /// Ответить на сообщение
    pub fn reply(
        &self,
        original: &AgentMessage,
        from: &str,
        status: &str,
        data: serde_json::Value,
    ) {
        let response = AgentMessage::new_response(
            &original.msg_id,
            from,
            &original.from,
            &original.channel,
            status,
            data,
        );
        let _ = self.send(&response, 0);
    }

    /// Разослать broadcast всем агентам в канале
    pub fn broadcast(&self, from: &str, channel: &str, topic: &str, payload: serde_json::Value) {
        let msg = AgentMessage::new_broadcast(from, channel, topic, payload);
        let _ = self.send(&msg, 0);
    }

    /// Команда для обработки agent-to-agent сообщений из чата
    pub fn parse_agent_command(input: &str) -> Option<AgentMessage> {
        let input = input.trim();
        if let Some(body) = input.strip_prefix("@agent ") {
            let parts: Vec<&str> = body.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                let target = parts[0];
                let action = parts[1];
                let payload = if parts.len() >= 3 {
                    serde_json::from_str(parts[2]).unwrap_or(serde_json::json!({"text": parts[2]}))
                } else {
                    serde_json::json!({"text": action})
                };
                return Some(AgentMessage::new_request(
                    "human", target, "chat", action, payload,
                ));
            }
        }
        if let Some(body) = input.strip_prefix("@all ") {
            let parts: Vec<&str> = body.splitn(2, ' ').collect();
            let topic = parts[0];
            let payload = if parts.len() >= 2 {
                serde_json::from_str(parts[1]).unwrap_or(serde_json::json!({"text": parts[1]}))
            } else {
                serde_json::json!({})
            };
            return Some(AgentMessage::new_broadcast("human", "chat", topic, payload));
        }
        None
    }

    pub fn summary(&self) -> String {
        let acl = &self.acl;
        format!("{}\n\n{}", acl.summary(), "🤖 @agent <id> <action> [json] — послать агенту\n  @all <topic> [json] — broadcast всем")
    }
}
