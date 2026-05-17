use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub bridges: Vec<String>,
    #[serde(default)]
    pub llm_providers: Vec<String>,
}

pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &PluginManifest;
    fn name(&self) -> &str { self.manifest().name.as_str() }

    fn on_agent_open(&self, agent_id: &str, skill: &str) -> Result<()> { Ok(()) }
    fn on_agent_close(&self, agent_id: &str, skill: &str, findings: u64) -> Result<()> { Ok(()) }
    fn on_llm_call(&self, prompt: &str, model: &str) -> Option<String> { None }
    fn on_tool_call(&self, tool: &str, args: &str) -> Option<String> { None }
    fn on_message(&self, text: &str, channel: &str) -> Option<String> { None }
    fn on_bridge_call(&self, bridge: &str, input: &str) -> Option<String> { None }
}

pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn Plugin>>,
    plugins_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new(plugins_dir: &Path) -> Self {
        PluginRegistry {
            plugins: HashMap::new(),
            plugins_dir: plugins_dir.to_path_buf(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let name = plugin.name().to_string();
        info!("Plugin registered: {} v{}", name, plugin.manifest().version);
        self.plugins.insert(name, plugin);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Plugin>> {
        self.plugins.get(name)
    }

    pub fn list(&self) -> Vec<&PluginManifest> {
        self.plugins.values().map(|p| p.manifest()).collect()
    }

    pub fn load_dir(&mut self) -> usize {
        let dir = &self.plugins_dir;
        if !dir.exists() {
            return 0;
        }
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() { continue; }
                let manifest_path = path.join("plugin.toml");
                if !manifest_path.exists() { continue; }
                if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = toml::from_str::<PluginManifest>(&content) {
                        info!("Loaded plugin: {} v{} from {:?}", manifest.name, manifest.version, path);
                        count += 1;
                    }
                }
            }
        }
        count
    }

    // Hook dispatch
    pub fn dispatch_agent_open(&self, agent_id: &str, skill: &str) {
        for p in self.plugins.values() {
            if let Err(e) = p.on_agent_open(agent_id, skill) {
                warn!("Plugin '{}' agent_open error: {}", p.name(), e);
            }
        }
    }

    pub fn dispatch_agent_close(&self, agent_id: &str, skill: &str, findings: u64) {
        for p in self.plugins.values() {
            if let Err(e) = p.on_agent_close(agent_id, skill, findings) {
                warn!("Plugin '{}' agent_close error: {}", p.name(), e);
            }
        }
    }

    pub fn dispatch_llm_call(&self, prompt: &str, model: &str) -> Option<String> {
        for p in self.plugins.values() {
            if let Some(result) = p.on_llm_call(prompt, model) {
                return Some(result);
            }
        }
        None
    }

    pub fn dispatch_tool_call(&self, tool: &str, args: &str) -> Option<String> {
        for p in self.plugins.values() {
            if let Some(result) = p.on_tool_call(tool, args) {
                return Some(result);
            }
        }
        None
    }

    pub fn dispatch_message(&self, text: &str, channel: &str) -> Option<String> {
        for p in self.plugins.values() {
            if let Some(result) = p.on_message(text, channel) {
                return Some(result);
            }
        }
        None
    }

    pub fn dispatch_bridge_call(&self, bridge: &str, input: &str) -> Option<String> {
        for p in self.plugins.values() {
            if let Some(result) = p.on_bridge_call(bridge, input) {
                return Some(result);
            }
        }
        None
    }
}

// Example plugin: LogPlugin — просто логирует все события
pub struct LogPlugin {
    manifest: PluginManifest,
}

impl LogPlugin {
    pub fn new() -> Self {
        LogPlugin {
            manifest: PluginManifest {
                name: "logger".into(),
                version: "1.0.0".into(),
                description: "Логирует все события агентов".into(),
                author: Some("system".into()),
                hooks: vec!["agent_open".into(), "agent_close".into()],
                tools: vec![],
                bridges: vec![],
                llm_providers: vec![],
            },
        }
    }
}

impl Plugin for LogPlugin {
    fn manifest(&self) -> &PluginManifest { &self.manifest }
    fn on_agent_open(&self, agent_id: &str, skill: &str) -> Result<()> {
        info!("[Plugin:logger] Agent opened: {} (skill: {})", agent_id, skill);
        Ok(())
    }
    fn on_agent_close(&self, agent_id: &str, skill: &str, findings: u64) -> Result<()> {
        info!("[Plugin:logger] Agent closed: {} (skill: {}, findings: {})", agent_id, skill, findings);
        Ok(())
    }
}

// Example: RateLimitPlugin — ограничивает вызовы LLM
pub struct RateLimitPlugin {
    manifest: PluginManifest,
    max_calls_per_minute: u32,
    call_count: std::sync::Mutex<(u32, std::time::Instant)>,
}

impl RateLimitPlugin {
    pub fn new(max_calls: u32) -> Self {
        RateLimitPlugin {
            manifest: PluginManifest {
                name: "rate-limiter".into(),
                version: "1.0.0".into(),
                description: "Ограничивает вызовы LLM".into(),
                author: Some("system".into()),
                hooks: vec!["llm_call".into()],
                tools: vec![],
                bridges: vec![],
                llm_providers: vec![],
            },
            max_calls_per_minute: max_calls,
            call_count: std::sync::Mutex::new((0, std::time::Instant::now())),
        }
    }
}

impl Plugin for RateLimitPlugin {
    fn manifest(&self) -> &PluginManifest { &self.manifest }
    fn on_llm_call(&self, _prompt: &str, _model: &str) -> Option<String> {
        let mut state = self.call_count.lock().unwrap();
        if state.1.elapsed().as_secs() > 60 {
            state.0 = 0;
            state.1 = std::time::Instant::now();
        }
        state.0 += 1;
        if state.0 > self.max_calls_per_minute {
            warn!("[Plugin:rate-limiter] LLM call rejected: too many requests");
            return Some("Rate limit exceeded. Try again in a minute.".into());
        }
        None
    }
}
