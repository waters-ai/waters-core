use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: String,
    pub channel: String,
    pub seq: u64,
    pub timestamp: String,
    pub msg_type: String,
    pub from: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub name: String,
    pub visibility: String,
    pub created_at: String,
    pub created_by: String,
    pub message_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    pub visibility: String,
    pub allowed_nodes: Vec<String>,
}

pub struct Channel {
    pub info: ChannelInfo,
    pub acl: AclRule,
    path: PathBuf,
    messages: Arc<Mutex<Vec<ChannelMessage>>>,
}

pub struct ChannelManager {
    channels: HashMap<String, Channel>,
    db_path: PathBuf,
    node_id: String,
}

impl ChannelManager {
    pub fn new(db_path: &Path, node_id: &str) -> Self {
        std::fs::create_dir_all(db_path).ok();
        ChannelManager {
            channels: HashMap::new(),
            db_path: db_path.to_path_buf(),
            node_id: node_id.to_string(),
        }
    }

    pub fn create(&mut self, name: &str, visibility: &str) -> anyhow::Result<()> {
        if self.channels.contains_key(name) {
            return Err(anyhow::anyhow!("Channel '{}' already exists", name));
        }
        let ch_path = self.db_path.join(format!("{}.wal", name));
        let channel = Channel {
            info: ChannelInfo {
                name: name.to_string(),
                visibility: visibility.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                created_by: self.node_id.clone(),
                message_count: 0,
            },
            acl: AclRule {
                visibility: visibility.to_string(),
                allowed_nodes: vec![self.node_id.clone()],
            },
            path: ch_path,
            messages: Arc::new(Mutex::new(Vec::new())),
        };
        self.channels.insert(name.to_string(), channel);
        info!("Channel '{}' created (visibility: {})", name, visibility);
        Ok(())
    }

    pub async fn write(&mut self, channel: &str, msg_type: &str, from: &str, content: &str) -> anyhow::Result<ChannelMessage> {
        let ch = self.channels.get(channel)
            .ok_or_else(|| anyhow::anyhow!("Channel '{}' not found", channel))?;

        let seq = ch.info.message_count + 1;
        let msg = ChannelMessage {
            id: uuid::Uuid::new_v4().to_string(),
            channel: channel.to_string(),
            seq,
            timestamp: chrono::Utc::now().to_rfc3339(),
            msg_type: msg_type.to_string(),
            from: from.to_string(),
            content: content.to_string(),
        };

        // Append to WAL file
        let wal_path = &ch.path;
        let line = serde_json::to_string(&msg)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(wal_path)?
            .write_all(format!("{}\n", line).as_bytes())?;

        // Update in-memory
        if let Some(ch) = self.channels.get_mut(channel) {
            ch.info.message_count = seq;
            ch.messages.lock().await.push(msg.clone());
        }

        info!("Channel '{}' seq={} written", channel, seq);
        Ok(msg)
    }

    pub async fn read(&self, channel: &str, since_seq: u64) -> Vec<ChannelMessage> {
        if let Some(ch) = self.channels.get(channel) {
            let msgs = ch.messages.lock().await;
            msgs.iter().filter(|m| m.seq > since_seq).cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub async fn read_all(&self, channel: &str) -> Vec<ChannelMessage> {
        if let Some(ch) = self.channels.get(channel) {
            let msgs = ch.messages.lock().await;
            msgs.clone()
        } else {
            Vec::new()
        }
    }

    pub fn check_acl(&self, channel: &str, node_id: &str) -> bool {
        if let Some(ch) = self.channels.get(channel) {
            match ch.acl.visibility.as_str() {
                "open" => true,
                "closed" => ch.acl.allowed_nodes.contains(&node_id.to_string()),
                "private" => ch.acl.allowed_nodes.contains(&node_id.to_string()),
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn add_to_acl(&mut self, channel: &str, node_id: &str) -> bool {
        if let Some(ch) = self.channels.get_mut(channel) {
            if !ch.acl.allowed_nodes.contains(&node_id.to_string()) {
                ch.acl.allowed_nodes.push(node_id.to_string());
                info!("Node {} added to channel '{}' ACL", node_id, channel);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn set_visibility(&mut self, channel: &str, visibility: &str) -> anyhow::Result<()> {
        if let Some(ch) = self.channels.get_mut(channel) {
            ch.acl.visibility = visibility.to_string();
            ch.info.visibility = visibility.to_string();
            info!("Channel '{}' visibility set to {}", channel, visibility);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Channel '{}' not found", channel))
        }
    }

    pub fn list(&self) -> Vec<ChannelInfo> {
        self.channels.values().map(|c| c.info.clone()).collect()
    }

    pub fn list_names(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.channels.contains_key(name)
    }

    pub fn get_message_count(&self, name: &str) -> Result<u64, String> {
        self.channels.get(name)
            .map(|c| c.info.message_count)
            .ok_or_else(|| format!("Channel '{}' not found", name))
    }

    pub async fn import_messages(&mut self, name: &str, msgs: Vec<ChannelMessage>) -> anyhow::Result<u64> {
        if !self.channels.contains_key(name) {
            self.create(name, "open")?;
        }
        let mut imported = 0u64;
        if let Some(ch) = self.channels.get_mut(name) {
            let wal_path = ch.path.clone();
            let mut existing = ch.messages.lock().await;
            for msg in msgs {
                if !existing.iter().any(|m| m.id == msg.id) {
                    let line = serde_json::to_string(&msg)?;
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&wal_path)?
                        .write_all(format!("{}\n", line).as_bytes())?;
                    existing.push(msg);
                    imported += 1;
                }
            }
            ch.info.message_count = existing.len() as u64;
        }
        if imported > 0 {
            info!("Imported {} new messages to '{}'", imported, name);
        }
        Ok(imported)
    }

    pub async fn load_wal(&mut self, name: &str) -> anyhow::Result<u64> {
        let wal_path = self.db_path.join(format!("{}.wal", name));
        if !wal_path.exists() {
            return Ok(0);
        }
        let content = std::fs::read_to_string(&wal_path)?;
        let mut count = 0u64;
        if let Some(ch) = self.channels.get(name) {
            let mut msgs = ch.messages.lock().await;
            for line in content.lines() {
                if let Ok(msg) = serde_json::from_str::<ChannelMessage>(line) {
                    msgs.push(msg);
                    count += 1;
                }
            }
        }
        if let Some(ch) = self.channels.get_mut(name) {
            ch.info.message_count = count;
        }
        info!("Loaded {} messages from {}.wal", count, name);
        Ok(count)
    }
}
