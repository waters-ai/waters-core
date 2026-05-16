use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub node_name: String,
    pub version: String,
    pub public_addr: Option<SocketAddr>,
    pub started_at: String,
    pub uptime_secs: u64,
    pub subagents: u64,
    pub findings: u64,
    pub kafka_connected: bool,
    pub autonomy_level: u8,
    pub cargo_sent: u64,
    pub cargo_received: u64,
    pub cargo_pending: u64,
    pub last_sync_seq: u64,
    pub is_home_node: bool,
    pub fixed_ip: bool,
}

pub struct Node {
    identity: NodeIdentity,
    uptime_counter: AtomicU64,
}

impl Node {
    pub fn new(name: &str, id: Option<String>) -> Self {
        let node_id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let now = chrono::Utc::now().to_rfc3339();
        Node {
            identity: NodeIdentity {
                node_id,
                node_name: name.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                public_addr: None,
                started_at: now,
                uptime_secs: 0,
                subagents: 0,
                findings: 0,
                kafka_connected: false,
                autonomy_level: 0,
                cargo_sent: 0,
                cargo_received: 0,
                cargo_pending: 0,
                last_sync_seq: 0,
                is_home_node: false,
                fixed_ip: false,
            },
            uptime_counter: AtomicU64::new(0),
        }
    }

    pub fn id(&self) -> &str { &self.identity.node_id }
    pub fn name(&self) -> &str { &self.identity.node_name }
    pub fn identity(&self) -> &NodeIdentity { &self.identity }

    pub fn tick(&mut self) {
        self.uptime_counter.fetch_add(1, Ordering::Relaxed);
        self.identity.uptime_secs = self.uptime_counter.load(Ordering::Relaxed);
    }

    pub fn set_subagents(&mut self, count: u64) { self.identity.subagents = count; }
    pub fn set_findings(&mut self, count: u64) { self.identity.findings = count; }
    pub fn set_kafka(&mut self, connected: bool) { self.identity.kafka_connected = connected; }
    pub fn set_autonomy(&mut self, level: u8) { self.identity.autonomy_level = level; }

    pub fn save_state(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(&self.identity)?;
        std::fs::write(path, json)?;
        info!("Node state saved to {}", path.display());
        Ok(())
    }

    pub fn load_state(path: &Path) -> anyhow::Result<Option<String>> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let ident: NodeIdentity = serde_json::from_str(&content)?;
            info!("Node state loaded: {} ({})", ident.node_name, ident.node_id);
            Ok(Some(ident.node_id))
        } else {
            Ok(None)
        }
    }

    pub fn announce(&self) -> serde_json::Value {
        serde_json::json!({
            "event": "node.announce",
            "node_id": self.identity.node_id,
            "node_name": self.identity.node_name,
            "version": self.identity.version,
            "uptime": self.identity.uptime_secs,
            "subagents": self.identity.subagents,
            "findings": self.identity.findings,
            "autonomy": self.identity.autonomy_level,
            "kafka": self.identity.kafka_connected,
            "cargo_sent": self.identity.cargo_sent,
            "cargo_received": self.identity.cargo_received,
            "cargo_pending": self.identity.cargo_pending,
            "last_sync_seq": self.identity.last_sync_seq,
            "is_home_node": self.identity.is_home_node,
            "fixed_ip": self.identity.fixed_ip,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })
    }
}
