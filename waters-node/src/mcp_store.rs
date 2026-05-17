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
    pub async fn install(&mut self, name: &str) -> Result<()> {
        let install_dir = self.skills_dir.join(name);
        if install_dir.exists() {
            return Err(anyhow::anyhow!("Skill '{}' already installed", name));
        }

        // Find meta from taps
        let meta = self.search(name).await.into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found in any tap", name))?;

        // Create dir and write meta
        fs::create_dir_all(&install_dir)?;
        let meta_path = install_dir.join("meta.json");
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        fs::write(install_dir.join("SKILL.md"), format!("# {}\n\n{}\n\n## Tools\n{}",
            meta.name, meta.description,
            meta.tools.iter().map(|t| format!("- `{}`", t)).collect::<Vec<_>>().join("\n")))?;

        // Record installed
        if !self.config.installed.contains(&name.to_string()) {
            self.config.installed.push(name.to_string());
        }
        self.save()?;

        info!("McpStore: installed skill '{}' from {}", name, meta.source_url.as_deref().unwrap_or("unknown"));
        Ok(())
    }

    pub fn uninstall(&mut self, name: &str) -> Result<()> {
        let install_dir = self.skills_dir.join(name);
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir)?;
        }
        self.config.installed.retain(|s| s != name);
        self.save()?;
        info!("McpStore: uninstalled skill '{}'", name);
        Ok(())
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
    }

    pub fn list_taps(&self) -> &[String] {
        &self.config.taps
    }

    async fn fetch_from_tap(&self, tap: &str, _query: &str) -> Option<Vec<McpSkillMeta>> {
        // For now, return empty — real HTTP fetch will be added
        None
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.config_path, serde_json::to_string_pretty(&self.config)?)?;
        Ok(())
    }

    pub fn summary(&self) -> String {
        let mut out = format!("📦 MCP Store — {} installed, {} taps\n", self.config.installed.len(), self.config.taps.len());
        for s in &self.config.installed {
            out.push_str(&format!("  ✅ {}\n", s));
        }
        out
    }
}
