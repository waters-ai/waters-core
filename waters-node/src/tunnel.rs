use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelMode {
    Direct,      // прямое P2P (оба публичные IP)
    MasterSlave, // slave подключается к мастеру
    Relay,       // через relay-сервер (hub-and-spoke)
    WireGuard,   // L3 туннель через WG
    Dtn,         // DTN для прерывистой связи
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelPeer {
    pub name: String,
    pub address: String,
    pub mode: TunnelMode,
    pub public_key: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: u32,
}

impl TunnelPeer {
    pub fn new(name: &str, address: &str, mode: TunnelMode) -> Self {
        TunnelPeer {
            name: name.to_string(),
            address: address.to_string(),
            mode,
            public_key: None,
            endpoint: None,
            allowed_ips: vec!["0.0.0.0/0".into()],
            persistent_keepalive: 25,
        }
    }
}

pub struct TunnelManager {
    peers: HashMap<String, TunnelPeer>,
    local_name: String,
}

impl TunnelManager {
    pub fn new(local_name: &str) -> Self {
        TunnelManager {
            peers: HashMap::new(),
            local_name: local_name.to_string(),
        }
    }

    pub fn add_peer(&mut self, name: &str, address: &str, mode: TunnelMode) {
        self.peers
            .insert(name.to_string(), TunnelPeer::new(name, address, mode));
        info!("Tunnel: peer '{}' added ({} via {:?})", name, address, mode);
    }

    pub fn remove_peer(&mut self, name: &str) {
        self.peers.remove(name);
        info!("Tunnel: peer '{}' removed", name);
    }

    pub fn list_peers(&self) -> Vec<&TunnelPeer> {
        self.peers.values().collect()
    }

    pub fn get(&self, name: &str) -> Option<&TunnelPeer> {
        self.peers.get(name)
    }

    pub fn relay_port(&self) -> u16 {
        42072
    }

    pub fn summary(&self) -> String {
        let mut out = format!("🔌 Tunnel Manager — '{}'\n", self.local_name);
        for peer in self.peers.values() {
            let icon = match peer.mode {
                TunnelMode::Direct => "🔗",
                TunnelMode::MasterSlave => "🔗⬆",
                TunnelMode::Relay => "🔄",
                TunnelMode::WireGuard => "🔒",
                TunnelMode::Dtn => "📡",
            };
            out.push_str(&format!(
                "  {} {} → {} ({:?})\n",
                icon, peer.name, peer.address, peer.mode
            ));
        }
        out
    }
}

/// Генератор человекочитаемых имён для нод
pub fn suggest_node_name(seed: &str) -> String {
    let names = vec![
        "Вася",
        "Петя",
        "Маша",
        "Даша",
        "Саша",
        "Женя",
        "Работа",
        "Дача",
        "Дом",
        "Офис",
        "Сервер",
        "Хаб",
        "Студия",
        "Кухня",
        "Гараж",
        "Лаба",
        "Поле",
        "Холм",
    ];
    let idx = seed.bytes().fold(0u8, |a, b| a.wrapping_add(b)) as usize;
    let name = names.get(idx % names.len()).unwrap_or(&"Нода");
    let suffix = &seed[..4.min(seed.len())];
    format!("{}-{}", name, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_name() {
        let name = suggest_node_name("abc123");
        assert!(!name.is_empty());
        assert!(name.contains('-'));
        println!("Suggested name: {}", name);
    }

    #[test]
    fn test_tunnel_manager() {
        let mut tm = TunnelManager::new("Тест");
        tm.add_peer("Сервер", "171.22.180.177:42069", TunnelMode::Relay);
        tm.add_peer("Дача", "192.168.1.100:42070", TunnelMode::MasterSlave);
        assert_eq!(tm.list_peers().len(), 2);
        println!("{}", tm.summary());
    }
}
