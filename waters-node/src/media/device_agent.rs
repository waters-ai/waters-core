/// Device Agent Bridge — встраивает устройства как агентов в групповой чат
use crate::agent_chat::{AgentChat, AgentMessage};
use std::sync::Arc;
use tracing::info;

pub struct DeviceAgent {
    pub device_type: &'static str,
    pub device_id: String,
    pub display_name: String,
    pub channel: String,
    pub agent_chat: Arc<AgentChat>,
}

impl DeviceAgent {
    pub fn new(
        device_type: &'static str,
        device_id: &str,
        display_name: &str,
        channel: &str,
        kvstore: Arc<crate::store::KvStore>,
    ) -> Self {
        let agent_chat = Arc::new(AgentChat::new(kvstore));
        DeviceAgent {
            device_type,
            device_id: device_id.to_string(),
            display_name: display_name.to_string(),
            channel: channel.to_string(),
            agent_chat,
        }
    }

    /// Отправить сообщение в групповой канал
    pub fn say(&self, text: &str) {
        let msg = AgentMessage::new_broadcast(
            &self.device_id,
            &self.channel,
            "device_status",
            serde_json::json!({"from": self.display_name, "text": text}),
        );
        let _ = self.agent_chat.send(&msg, 0);
        info!("DeviceAgent [{}]: {}", self.display_name, text);
    }

    /// Оповестить о тревоге
    pub fn alert(&self, level: &str, message: &str) {
        let payload = serde_json::json!({
            "alert": level,
            "device": self.display_name,
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let msg = AgentMessage::new_broadcast(&self.device_id, &self.channel, "alert", payload);
        let _ = self.agent_chat.send(&msg, 0);
        info!(
            "🚨 DeviceAgent ALERT [{}]: {} — {}",
            level, self.display_name, message
        );
    }

    /// Ответить на запрос из чата
    pub fn reply(&self, to: &str, text: &str) {
        let msg = AgentMessage::new_request(
            &self.device_id,
            to,
            &self.channel,
            "response",
            serde_json::json!({"text": text}),
        );
        let _ = self.agent_chat.send(&msg, 0);
    }

    /// Зарегистрировать устройство как агента
    pub fn register(&self) {
        self.say(&format!(
            "✅ {} онлайн (канал: {})",
            self.display_name, self.channel
        ));
    }
}
