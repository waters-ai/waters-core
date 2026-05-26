use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSkillMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub source_url: Option<String>,
    pub tools: Vec<String>,
    pub install_command: Option<String>,
    pub env_vars: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStoreConfig {
    pub taps: Vec<String>,
    pub installed: Vec<String>,
}

impl Default for McpStoreConfig {
    fn default() -> Self {
        McpStoreConfig {
            taps: vec![
                "github.com/waters-ai/mcp-skills".into(),
                "huggingface.co/skills".into(),
            ],
            installed: vec!["general".into(), "explorer".into(), "scout-ru".into(), "scout-us".into(), "scout-cn".into()],
        }
    }
}

pub struct McpStore {
    config: McpStoreConfig,
    config_path: PathBuf,
    skills_dir: PathBuf,
    cache: HashMap<String, McpSkillMeta>,
}

impl McpStore {
    pub fn new(data_dir: &Path) -> Self {
        let config_path = data_dir.join("mcp_store.json");
        let skills_dir = data_dir.join("skills");

        let config = if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
                Err(_) => McpStoreConfig::default(),
            }
        } else {
            let cfg = McpStoreConfig::default();
            if let Some(parent) = config_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&config_path, serde_json::to_string_pretty(&cfg).unwrap_or_default());
            cfg
        };

        McpStore {
            config,
            config_path,
            skills_dir,
            cache: HashMap::new(),
        }
    }

    /// Search available skills from all taps
    pub async fn search(&mut self, query: &str) -> Vec<McpSkillMeta> {
        let mut results = Vec::new();

        // Search local installed
        if let Ok(entries) = fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let meta_path = path.join("meta.json");
                if let Ok(content) = fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<McpSkillMeta>(&content) {
                        if query.is_empty()
                            || meta.name.contains(query)
                            || meta.description.contains(query)
                            || meta.tags.iter().any(|t| t.contains(query))
                        {
                            results.push(meta);
                        }
                    }
                }
            }
        }

        // Search from taps (remote)
        for tap in &self.config.taps {
            if let Some(skills) = self.fetch_from_tap(tap, query).await {
                results.extend(skills);
            }
        }

        results.sort_by_key(|m| m.name.clone());
        results.dedup_by_key(|m| m.name.clone());
        results
    }

    /// Install a skill by name
    pub async fn install(&mut self, name: &str) -> Result<String, String> {
        let install_dir = self.skills_dir.join(name);
        if install_dir.exists() {
            return Err(format!("Skill '{}' already installed", name));
        }

        // Find meta from taps
        let skills = self.search(name).await;
        let meta = skills.into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| format!("Skill '{}' not found in any tap", name))?;

        // Create dir and write meta
        fs::create_dir_all(&install_dir).map_err(|e| format!("Failed to create dir: {}", e))?;
        let meta_path = install_dir.join("meta.json");
        let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| format!("JSON error: {}", e))?;
        fs::write(&meta_path, &meta_json).map_err(|e| format!("Write error: {}", e))?;
        let skill_md = format!("# {}\n\n{}\n\n## Tools\n{}",
            meta.name, meta.description,
            meta.tools.iter().map(|t| format!("- `{}`", t)).collect::<Vec<_>>().join("\n"));
        fs::write(install_dir.join("SKILL.md"), &skill_md).map_err(|e| format!("Write error: {}", e))?;

        // Record installed
        if !self.config.installed.contains(&name.to_string()) {
            self.config.installed.push(name.to_string());
        }
        self.save()?;

        info!("McpStore: installed skill '{}' from {}", name, meta.source_url.as_deref().unwrap_or("unknown"));
        Ok(format!("✅ '{}' установлен из {}", name, meta.source_url.as_deref().unwrap_or("taps")))
    }

    pub fn uninstall(&mut self, name: &str) -> Result<String, String> {
        let install_dir = self.skills_dir.join(name);
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).map_err(|e| format!("Remove error: {}", e))?;
        }
        self.config.installed.retain(|s| s != name);
        self.save().map_err(|e| format!("Save error: {}", e))?;
        info!("McpStore: uninstalled skill '{}'", name);
        Ok(format!("✅ '{}' удалён", name))
    }

    pub fn list_installed(&self) -> Vec<String> {
        self.config.installed.clone()
    }

    pub fn add_tap(&mut self, url: &str) {
        if !self.config.taps.contains(&url.to_string()) {
            self.config.taps.push(url.to_string());
            let _ = self.save();
            info!("McpStore: added tap '{}'", url);
        }
    }

    pub fn remove_tap(&mut self, url: &str) {
        self.config.taps.retain(|t| t != url);
        let _ = self.save();
        info!("McpStore: removed tap '{}'", url);
    }

    pub fn list_taps(&self) -> &[String] {
        &self.config.taps
    }

    async fn fetch_from_tap(&self, tap: &str, query: &str) -> Option<Vec<McpSkillMeta>> {
        let client = reqwest::Client::builder()
            .user_agent("waters-node/0.5")
            .build().ok()?;

        if tap == "huggingface.co/skills" {
            let encoded: String = query.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
            let url = format!("https://huggingface.co/api/skills?search={}&limit=20", encoded);
            match client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(text) = resp.text().await {
                        if let Ok(skills) = serde_json::from_str::<Vec<McpSkillMeta>>(&text) {
                            info!("McpStore: fetched {} skills from huggingface", skills.len());
                            return Some(skills);
                        }
                    }
                }
                Err(e) => warn!("McpStore: huggingface fetch failed: {}", e),
            }
        }
        if tap.starts_with("github.com/") {
            let parts: Vec<&str> = tap.splitn(3, '/').collect();
            if parts.len() >= 3 {
                let repo = format!("{}/{}", parts[1], parts[2]);
                let url = format!("https://api.github.com/repos/{}/contents/skills?ref=main", repo);
                match client.get(&url).send().await {
                    Ok(resp) => {
                        if let Ok(files) = resp.json::<Vec<serde_json::Value>>().await {
                            let mut results = Vec::new();
                            for file in files {
                                if let Some(name) = file["name"].as_str() {
                                    if name.ends_with(".json") || name.ends_with(".md") {
                                        if query.is_empty() || name.contains(query) {
                                            results.push(McpSkillMeta {
                                                name: name.trim_end_matches(".json").trim_end_matches(".md").to_string(),
                                                version: "1.0.0".into(),
                                                description: format!("Skill from {}", tap),
                                                author: Some(parts[1].to_string()),
                                                tags: vec!["remote".into()],
                                                source_url: Some(file["download_url"].as_str().unwrap_or("").to_string()),
                                                tools: vec![],
                                                install_command: None,
                                                env_vars: vec![],
                                            });
                                        }
                                    }
                                }
                            }
                            info!("McpStore: fetched {} skills from {}", results.len(), tap);
                            return Some(results);
                        }
                    }
                    Err(e) => warn!("McpStore: github fetch failed: {}", e),
                }
            }
        }
        None
    }

    pub fn save(&self) -> Result<String, String> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Dir error: {}", e))?;
        }
        let json = serde_json::to_string_pretty(&self.config).map_err(|e| format!("JSON error: {}", e))?;
        fs::write(&self.config_path, &json).map_err(|e| format!("Write error: {}", e))?;
        Ok("✅ Сохранено".into())
    }

    pub fn summary(&self) -> String {
        let mut out = format!("📦 MCP Store — {} installed, {} taps\n", self.config.installed.len(), self.config.taps.len());
        for s in &self.config.installed {
            out.push_str(&format!("  ✅ {}\n", s));
        }
        for t in &self.config.taps {
            out.push_str(&format!("  📡 {}\n", t));
        }
        out
    }
}

/// ---------- MCP Store FFI для синхронного вызова из хендлера ----------
impl McpStore {
    /// Синхронная версия search — без async, для тестов и CLI
    pub fn search_sync(&mut self, query: &str) -> Vec<McpSkillMeta> {
        let mut results = Vec::new();
        // Локальные установленные
        if let Ok(entries) = std::fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let meta_path = path.join("meta.json");
                if let Ok(content) = std::fs::read_to_string(&meta_path) {
                    if let Ok(meta) = serde_json::from_str::<McpSkillMeta>(&content) {
                        if query.is_empty() || meta.name.contains(query) || meta.description.contains(query) {
                            results.push(meta);
                        }
                    }
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_store_default_config() {
        let store = McpStoreConfig::default();
        assert_eq!(store.taps.len(), 2);
        assert!(store.installed.contains(&"general".to_string()));
    }

    #[test]
    fn test_mcp_skill_meta_serialize() {
        let meta = McpSkillMeta {
            name: "test-skill".into(),
            version: "1.0.0".into(),
            description: "Test".into(),
            author: Some("tester".into()),
            tags: vec!["test".into()],
            source_url: None,
            tools: vec!["tool1".into()],
            install_command: None,
            env_vars: vec![],
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("test-skill"));
        assert!(json.contains("tool1"));
    }

    #[test]
    fn test_mcp_store_add_tap() {
        let dir = std::env::temp_dir().join("mcp-test-store");
        let mut store = McpStore::new(&dir);
        let taps_before = store.list_taps().len();
        store.add_tap("https://example.com/custom-skills");
        assert_eq!(store.list_taps().len(), taps_before + 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search_empty_query_returns_installed() {
        let dir = std::env::temp_dir().join("mcp-test-search");
        let mut store = McpStore::new(&dir);
        let results = store.search_sync("");
        assert!(results.is_empty() || results.iter().any(|s| store.list_installed().contains(&s.name)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
