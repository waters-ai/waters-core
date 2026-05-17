use crate::group_chat::GroupChat;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

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
}

impl AgentChat {
    pub fn new(kvstore: std::sync::Arc<crate::store::KvStore>) -> Self {
        AgentChat { kvstore }
    }

    /// Отправить сообщение агенту — без человека в канале
    pub fn send(&self, msg: &AgentMessage, group_id: u8) -> Result<(), Box<dyn std::error::Error>> {
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
        let stream_key = format!("agent:{}:{}:*", channel, agent_id);
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            0
        };
        let kv = self.kvstore.select_db(db);
        let mut messages = Vec::new();

        // Читаем напрямую из stream, если ключ известен
        let direct_key = format!("agent:{}:{}:{}", channel, "*", agent_id);
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
        // Формат: @agent <id> <action> [json]
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
        // Формат: @all <topic> [json] — broadcast
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
        "🤖 Agent-to-Agent Chat:\n  @agent <id> <action> [json] — послать агенту\n  @all <topic> [json] — broadcast всем".to_string()
    }
}
