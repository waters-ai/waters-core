use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::store::KvStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessage {
    pub msg_id: String,
    pub msg_type: String,
    pub sender: String,
    pub sender_role: String,
    pub content: String,
    pub task_id: Option<String>,
    pub mentions: Vec<String>,
    pub ts: String,
}

impl GroupMessage {
    pub fn display_short(&self) -> String {
        let icon = match self.msg_type.as_str() {
            "chat" => "💬",
            "opinion" => "🧠",
            "task" => "📋",
            _ => "🔔",
        };
        let task = self
            .task_id
            .as_deref()
            .map(|t| format!(" [#{}]", &t[..8.min(t.len())]))
            .unwrap_or_default();
        format!(
            "{} [{}] {}{}: {}",
            icon,
            self.sender_role,
            &self.sender[..8.min(self.sender.len())],
            task,
            &self.content[..80.min(self.content.len())]
        )
    }
}

pub struct GroupChat {
    kvstore: Arc<KvStore>,
}

impl GroupChat {
    pub fn new(kvstore: Arc<KvStore>) -> Self {
        GroupChat { kvstore }
    }

    fn chat_key(g: u8) -> String {
        format!("group:{}:chat", g)
    }
    fn task_key(g: u8, t: &str) -> String {
        let s = if t.len() > 8 { &t[..8] } else { t };
        format!("group:{}:task:{}:chat", g, s)
    }
    fn opinion_key(g: u8, t: &str) -> String {
        format!("group:{}:opinions:{}", g, t)
    }

    fn db(g: u8) -> u8 {
        if g >= 1 && g <= 6 {
            g
        } else {
            0
        }
    }

    fn key_for(group_id: u8, task_id: Option<&str>) -> String {
        match task_id {
            Some(tid) if !tid.is_empty() => Self::task_key(group_id, tid),
            _ => Self::chat_key(group_id),
        }
    }

    pub fn post(
        &self,
        g: u8,
        sender: &str,
        role: &str,
        content: &str,
        mtype: &str,
        task_id: Option<&str>,
        mentions: &[String],
    ) -> Result<String> {
        let db = Self::db(g);
        let mid = uuid::Uuid::new_v4().to_string();
        let msg = GroupMessage {
            msg_id: mid.clone(),
            msg_type: mtype.to_string(),
            sender: sender.to_string(),
            sender_role: role.to_string(),
            content: content.to_string(),
            task_id: task_id.map(|s| s.to_string()),
            mentions: mentions.to_vec(),
            ts: Utc::now().to_rfc3339(),
        };
        let json = serde_json::to_string(&msg)?;
        let key = Self::key_for(g, task_id);
        self.kvstore
            .select_db(db)
            .xadd(&key, &[("msg_id", &mid), ("data", &json)], 1000)?;
        info!(
            "[g:{}] {}<{}>: {}",
            g,
            role,
            sender,
            &content[..content.len().min(60)]
        );
        Ok(mid)
    }

    pub fn host_say(&self, g: u8, text: &str, task_id: Option<&str>) -> Result<String> {
        self.post(g, "host", "host", text, "chat", task_id, &[])
    }

    pub fn agent_say(
        &self,
        g: u8,
        agent: &str,
        role: &str,
        text: &str,
        task_id: Option<&str>,
    ) -> Result<String> {
        self.post(g, agent, role, text, "chat", task_id, &[])
    }

    pub fn read(&self, g: u8, count: usize, task_id: Option<&str>) -> Result<Vec<GroupMessage>> {
        let db = Self::db(g);
        let key = Self::key_for(g, task_id);
        let entries = self.kvstore.select_db(db).xread_latest(&key, count)?;
        let mut msgs = Vec::new();
        for f in entries {
            if let Some(data) = f.get("data") {
                if let Ok(m) = serde_json::from_str::<GroupMessage>(data) {
                    msgs.push(m);
                }
            }
        }
        Ok(msgs)
    }

    pub fn submit_opinion(
        &self,
        g: u8,
        agent: &str,
        role: &str,
        task_id: &str,
        opinion: &str,
        conf: f64,
    ) -> Result<String> {
        let mid = self.post(g, agent, role, opinion, "opinion", Some(task_id), &[])?;
        let entry = serde_json::json!({"agent":agent,"role":role,"opinion":opinion,"confidence":conf,"task":task_id});
        self.kvstore.select_db(Self::db(g)).xadd(
            &Self::opinion_key(g, task_id),
            &[("mid", &mid), ("data", &entry.to_string())],
            100,
        )?;
        Ok(mid)
    }

    pub fn get_opinions(&self, g: u8, task_id: &str) -> Result<Vec<serde_json::Value>> {
        let entries = self
            .kvstore
            .select_db(Self::db(g))
            .xread_all(&Self::opinion_key(g, task_id))?;
        let mut v = Vec::new();
        for f in entries {
            if let Some(d) = f.get("data") {
                if let Ok(x) = serde_json::from_str(d) {
                    v.push(x);
                }
            }
        }
        Ok(v)
    }

    pub fn summary_for_llm(&self, g: u8, count: usize) -> String {
        let msgs = self.read(g, count, None).unwrap_or_default();
        if msgs.is_empty() {
            return format!("Группа {}: нет сообщений.", g);
        }
        let mut out = format!("📋 Группа #{} ({}):\n", g, msgs.len());
        for m in &msgs {
            out.push_str(&format!("  {}\n", m.display_short()));
        }
        out
    }
}
