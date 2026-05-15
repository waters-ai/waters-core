use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bridge {
    pub name: String,
    pub description: String,
    pub connected: bool,
    pub config_keys: Vec<String>,
    pub region: String,
}

impl Bridge {
    pub fn new(name: &str, desc: &str, region: &str, keys: Vec<&str>) -> Self {
        Bridge {
            name: name.to_string(),
            description: desc.to_string(),
            connected: false,
            config_keys: keys.iter().map(|s| s.to_string()).collect(),
            region: region.to_string(),
        }
    }
}

pub struct BridgeRegistry {
    pub bridges: HashMap<String, Bridge>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        let mut bridges = HashMap::new();
        for b in Self::list_available() {
            bridges.insert(b.name.clone(), b);
        }
        BridgeRegistry { bridges }
    }

    pub fn list_available() -> Vec<Bridge> {
        vec![
            Bridge::new("duckduckgo", "Поиск без ключа, все регионы", "all", vec![]),
            Bridge::new("yandex.search", "Поиск по RU-сегменту (ключ Yandex.XML)", "ru", vec!["api_key", "user"]),
            Bridge::new("yandex.gpt", "AI-валидация RU (ключ YandexGPT)", "ru", vec!["api_key"]),
            Bridge::new("baidu.search", "Поиск по CN-сегменту (ключ Baidu)", "cn", vec!["api_key"]),
            Bridge::new("notebooklm", "Google NotebookLM валидация (cookie)", "all", vec!["cookie"]),
            Bridge::new("telegram", "Telegram бот (bot token)", "all", vec!["token"]),
        Bridge::new("notebooklm", "Google NotebookLM — AI валидация", "all", vec!["cookie"]),
        Bridge::new("obsidian", "Obsidian vault — заметки и база знаний", "all", vec!["vault_path"]),
        Bridge::new("chromadb", "ChromaDB — векторная память (238 hub)", "all", vec!["url"]),
        Bridge::new("lightrag", "LightRAG — граф знаний", "all", vec!["url"]),
        ]
    }

    pub fn connect(&mut self, name: &str, config: HashMap<String, String>) -> bool {
        if let Some(bridge) = self.bridges.get_mut(name) {
            // Проверяем что все ключи предоставлены
            for key in &bridge.config_keys {
                if !config.contains_key(key) {
                    return false;
                }
            }
            bridge.connected = true;
            true
        } else {
            false
        }
    }

    pub fn disconnect(&mut self, name: &str) -> bool {
        if let Some(bridge) = self.bridges.get_mut(name) {
            bridge.connected = false;
            true
        } else {
            false
        }
    }

    pub fn is_connected(&self, name: &str) -> bool {
        self.bridges.get(name).map(|b| b.connected).unwrap_or(false)
    }

    pub fn list(&self) -> Vec<&Bridge> {
        self.bridges.values().collect()
    }

    pub fn list_connected(&self) -> Vec<&Bridge> {
        self.bridges.values().filter(|b| b.connected).collect()
    }

    pub fn list_by_region(&self, region: &str) -> Vec<&Bridge> {
        self.bridges.values().filter(|b| b.region == region || b.region == "all").collect()
    }
}
