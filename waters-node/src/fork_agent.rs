/// Fork Agent — нода создаёт форки под разные задачи
pub const GITHUB_ORG: &str = "github.com/waters-ai";

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkProfile { Agriculture, VideoStudio, SmartHome, Factory, Minimal, Full }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkCompat {
    pub fork: ForkProfile,
    pub shared_features: Vec<String>,
    pub unique_features: Vec<String>,
    pub incompatible_with: Vec<ForkProfile>,
}

fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

impl ForkProfile {
    pub fn name(&self) -> &str {
        match self {
            Self::Agriculture => "waters-node-field", Self::VideoStudio => "waters-node-studio",
            Self::SmartHome => "waters-node-home", Self::Factory => "waters-node-factory",
            Self::Minimal => "waters-node-core", Self::Full => "waters-node",
        }
    }
    pub fn repo_url(&self) -> String { format!("{}/{}", GITHUB_ORG, self.name()) }
    pub fn description(&self) -> &str {
        match self {
            Self::Agriculture => "🌾 Для фермеров: поле, техника, дроны, погода",
            Self::VideoStudio => "🎬 Для студий: камеры, NDI, OBS, RTMP, микшер",
            Self::SmartHome => "🏠 Для дома: дети, роботы, обучение, голос",
            Self::Factory => "🏭 Для заводов: ПЛК, конвейеры, роботы, OPC UA",
            Self::Minimal => "⚙️ Ядро: P2P, чат, агенты",
            Self::Full => "🌊 Полный набор",
        }
    }
    pub fn included_skills(&self) -> Vec<&str> {
        match self {
            Self::Agriculture => vec!["general","explorer","weather","soil","drone"],
            Self::VideoStudio => vec!["general","camera-operator","streamer","video-editor"],
            Self::SmartHome => vec!["general","smarthome-agent","robot-agent","explorer"],
            Self::Factory => vec!["general","robot-agent","explorer","scout-ru"],
            Self::Minimal => vec!["general"],
            Self::Full => vec!["general","explorer","planner","implementer","reviewer","verifier",
                              "weather","soil","drone","camera-operator","streamer","robot-agent","smarthome-agent"],
        }
    }
    pub fn compatibility(&self) -> ForkCompat {
        let shared = s(&["P2P gossip","SubAgent lifecycle","ACL","@agent","i18n","Security YASA",
            "Presence","Health endpoint","Contacts","DND/SOS","Self-improve","Cron","Push"]);
        match self {
            Self::Agriculture => ForkCompat { fork: self.clone(), shared_features: shared,
                unique_features: s(&["RTK-GPS","MAVLink","NPK","NDVI","MQTT"]),
                incompatible_with: vec![ForkProfile::VideoStudio, ForkProfile::SmartHome] },
            Self::VideoStudio => ForkCompat { fork: self.clone(), shared_features: shared,
                unique_features: s(&["NDI","OBS","RTMP","PTZ","DVR"]),
                incompatible_with: vec![ForkProfile::Agriculture, ForkProfile::Factory] },
            Self::SmartHome => ForkCompat { fork: self.clone(), shared_features: shared,
                unique_features: s(&["Голос","Сценарии","Дети","Роботы","MQTT"]),
                incompatible_with: vec![ForkProfile::Factory] },
            Self::Factory => ForkCompat { fork: self.clone(), shared_features: shared,
                unique_features: s(&["OPC UA","ПЛК","Конвейер","Печь","Брак"]),
                incompatible_with: vec![ForkProfile::SmartHome, ForkProfile::Agriculture] },
            Self::Minimal => ForkCompat { fork: self.clone(), shared_features: shared,
                unique_features: vec![], incompatible_with: vec![] },
            Self::Full => ForkCompat { fork: self.clone(), shared_features: shared,
                unique_features: s(&["Все модули"]), incompatible_with: vec![] },
        }
    }
}

pub struct ForkManager { pub current: ForkProfile, pub workspace: PathBuf }

impl ForkManager {
    pub fn new(profile: ForkProfile) -> Self { ForkManager { current: profile, workspace: PathBuf::from(".") } }

    pub fn create_fork(&self, profile: &ForkProfile) -> Result<String, String> {
        info!("ForkManager: creating fork '{}'", profile.name());
        let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
        if token.is_empty() { return Err("GITHUB_TOKEN не задан".into()); }
        let org = GITHUB_ORG.trim_start_matches("github.com/");
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({"name": profile.name(), "description": profile.description(),
            "private": false, "auto_init": true});
        match client.post(&format!("https://api.github.com/orgs/{}/repos", org))
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "waters-node/0.5").json(&body).send()
        {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 422 => {
                Ok(format!("✅ Форк создан: {}/{}", GITHUB_ORG, profile.name()))
            }
            Ok(resp) => Err(format!("GitHub API error: {}", resp.status())),
            Err(e) => Err(format!("GitHub: {}", e)),
        }
    }

    pub fn analyze_common_release(&self) -> String {
        let c = self.current.compatibility();
        let mut out = format!("📊 Общий релиз для '{}':\n\n✅ Совместимые:\n", self.current.name());
        for f in &c.shared_features { out.push_str(&format!("  • {}\n", f)); }
        out.push_str("\n❌ Остаются в форке:\n");
        for f in &c.unique_features { out.push_str(&format!("  • {}\n", f)); }
        out
    }

    pub fn propose_release(&self) -> String {
        let c = self.current.compatibility();
        format!("🚀 Предложение: v0.5.{}-{} ({} shared → релиз, {} → форк)",
            if c.shared_features.len() > 5 { "1" } else { "0" },
            self.current.name().replace("waters-node-", ""),
            c.shared_features.len(), c.unique_features.len())
    }

    pub fn list_forks() -> Vec<ForkProfile> {
        vec![ForkProfile::Agriculture, ForkProfile::VideoStudio,
             ForkProfile::SmartHome, ForkProfile::Factory, ForkProfile::Minimal]
    }

    pub fn summary(&self) -> String {
        let mut out = format!("🍴 Текущий: {}\n", self.current.name());
        out.push_str(&format!("   📝 {}\n", self.current.description()));
        out.push_str("\n📦 Форки:\n");
        for f in Self::list_forks() {
            let c = f.compatibility();
            out.push_str(&format!("  {} — shared:{} unique:{}\n", f.name(), c.shared_features.len(), c.unique_features.len()));
        }
        out
    }
}
