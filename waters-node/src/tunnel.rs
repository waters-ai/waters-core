use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
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

/// Контактная книга — пользователь сам назначает имена пирам
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub node_id: String,  // реальный ID/адрес ноды
    pub nickname: String, // как пользователь его назвал
    pub group: String,    // группа (Работа, Друзья, Семья...)
    pub notes: String,    // заметки пользователя
    pub added_at: String,
}

impl Contact {
    pub fn new(node_id: &str, nickname: &str) -> Self {
        Contact {
            node_id: node_id.to_string(),
            nickname: nickname.to_string(),
            group: String::new(),
            notes: String::new(),
            added_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub struct ContactBook {
    contacts: Vec<Contact>,
    path: PathBuf,
}

impl ContactBook {
    pub fn new(data_dir: &Path) -> Self {
        let path = data_dir.join("contacts.json");
        let contacts = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            let defaults = vec![
                Contact::new("171.22.180.177:42069", "Хаб-177"),
                Contact::new("171.22.180.238:42069", "Хаб-238"),
            ];
            let _ = fs::write(
                &path,
                serde_json::to_string_pretty(&defaults).unwrap_or_default(),
            );
            defaults
        };
        ContactBook { contacts, path }
    }

    /// Добавить или обновить контакт
    pub fn set(&mut self, node_id: &str, nickname: &str, group: &str) {
        if let Some(existing) = self.contacts.iter_mut().find(|c| c.node_id == node_id) {
            existing.nickname = nickname.to_string();
            if !group.is_empty() {
                existing.group = group.to_string();
            }
            let _ = self.save();
            info!("ContactBook: updated '{}' → '{}'", node_id, nickname);
            return;
        }
        let contact = Contact::new(node_id, nickname);
        let mut c = contact;
        if !group.is_empty() {
            c.group = group.to_string();
        }
        self.contacts.push(c);
        let _ = self.save();
        info!("ContactBook: added '{}' as '{}'", node_id, nickname);
    }

    /// Получить имя по ID — если нет контакта, вернуть ID
    pub fn resolve(&self, node_id: &str) -> String {
        self.contacts
            .iter()
            .find(|c| c.node_id == node_id)
            .map(|c| c.nickname.clone())
            .unwrap_or_else(|| {
                // Если ID похож на IP:port — взять последние 8 символов
                if node_id.contains(':') {
                    let short = if node_id.len() > 8 {
                        &node_id[node_id.len() - 8..]
                    } else {
                        node_id
                    };
                    format!("peer-{}", short)
                } else {
                    node_id.to_string()
                }
            })
    }

    pub fn find(&self, nickname: &str) -> Option<&Contact> {
        self.contacts.iter().find(|c| c.nickname == nickname)
    }

    pub fn remove(&mut self, node_id: &str) {
        self.contacts.retain(|c| c.node_id != node_id);
        let _ = self.save();
    }

    pub fn list(&self) -> &[Contact] {
        &self.contacts
    }

    pub fn list_by_group(&self, group: &str) -> Vec<&Contact> {
        self.contacts.iter().filter(|c| c.group == group).collect()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(&self.contacts)?)?;
        Ok(())
    }

    pub fn summary(&self) -> String {
        let mut out = format!("📒 Контакты ({}):\n", self.contacts.len());
        for c in &self.contacts {
            let g = if c.group.is_empty() {
                String::new()
            } else {
                format!(" [{}]", c.group)
            };
            out.push_str(&format!(
                "  {} → {}{} {}\n",
                c.nickname,
                c.node_id,
                g,
                if c.notes.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", c.notes)
                }
            ));
        }
        out
    }
}
