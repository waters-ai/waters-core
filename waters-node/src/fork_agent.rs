/// Fork Agent — нода создаёт форки под разные задачи
/// Каждый форк самосовершенствуется в своей области.
/// Совместимые улучшения → общий релиз.
/// Несовместимые → остаются в форке.

/// GitHub организация — вшита в бинарник, форки всегда идут сюда
pub const GITHUB_ORG: &str = "github.com/waters-ai";

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForkProfile {
    Agriculture,
    VideoStudio,
    SmartHome,
    Factory,
    Minimal,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkCompat {
    pub fork: ForkProfile,
    pub shared_features: Vec<String>,
    pub unique_features: Vec<String>,
    pub incompatible_with: Vec<ForkProfile>,
}

impl ForkProfile {
    pub fn name(&self) -> &str {
        match self {
            ForkProfile::Agriculture => "waters-node-field",
            ForkProfile::VideoStudio => "waters-node-studio",
            ForkProfile::SmartHome => "waters-node-home",
            ForkProfile::Factory => "waters-node-factory",
            ForkProfile::Minimal => "waters-node-core",
            ForkProfile::Full => "waters-node",
        }
    }

    pub fn repo_url(&self) -> String {
        format!("{}/{}", GITHUB_ORG, self.name())
    }

    pub fn description(&self) -> &str {
        match self {
            ForkProfile::Agriculture => "🌾 Для фермеров: поле, техника, дроны, погода, IoT",
            ForkProfile::VideoStudio => "🎬 Для студий: камеры, NDI, OBS, RTMP, микшер",
            ForkProfile::SmartHome => "🏠 Для дома: дети, роботы, обучение, голос, сценарии",
            ForkProfile::Factory => "🏭 Для заводов: ПЛК, конвейеры, роботы, OPC UA",
            ForkProfile::Minimal => "⚙️ Ядро: P2P, чат, агенты — без зависимостей",
            ForkProfile::Full => "🌊 Полный набор",
        }
    }

    pub fn compatibility(&self) -> ForkCompat {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect();
        let shared = s(&[
            "P2P gossip",
            "SubAgent lifecycle",
            "ACL",
            "@agent протокол",
            "i18n",
            "Security YASA",
            "Presence",
            "Health endpoint",
            "Contacts",
            "DND/SOS",
            "Self-improve",
            "Cron",
            "Push",
        ]);
        match self {
            ForkProfile::Agriculture => ForkCompat {
                fork: self.clone(),
                shared_features: shared,
                unique_features: s(&[
                    "Трактор RTK-GPS",
                    "Дроны MAVLink",
                    "Почва NPK",
                    "NDVI",
                    "Погода полей",
                    "MQTT датчики",
                ]),
                incompatible_with: vec![ForkProfile::VideoStudio, ForkProfile::SmartHome],
            },
            ForkProfile::VideoStudio => ForkCompat {
                fork: self.clone(),
                shared_features: shared,
                unique_features: s(&["NDI-микшер", "OBS", "RTMP", "PTZ камеры", "DVR", "Монтаж"]),
                incompatible_with: vec![ForkProfile::Agriculture, ForkProfile::Factory],
            },
            ForkProfile::SmartHome => ForkCompat {
                fork: self.clone(),
                shared_features: shared,
                unique_features: s(&[
                    "Голос",
                    "Сценарии",
                    "Дети",
                    "Роботы",
                    "MQTT",
                    "Климат",
                    "Охрана",
                ]),
                incompatible_with: vec![ForkProfile::Factory],
            },
            ForkProfile::Factory => ForkCompat {
                fork: self.clone(),
                shared_features: shared,
                unique_features: s(&["OPC UA", "ПЛК", "Конвейер", "Печь", "Брак"]),
                incompatible_with: vec![ForkProfile::SmartHome, ForkProfile::Agriculture],
            },
            ForkProfile::Minimal => ForkCompat {
                fork: self.clone(),
                shared_features: shared,
                unique_features: vec![],
                incompatible_with: vec![],
            },
            ForkProfile::Full => ForkCompat {
                fork: self.clone(),
                shared_features: shared,
                unique_features: s(&["Все модули"]),
                incompatible_with: vec![],
            },
        }
    }

    pub fn included_skills(&self) -> Vec<&str> {
        match self {
            ForkProfile::Agriculture => vec![
                "general", "explorer", "weather", "soil", "drone", "scout-ru", "scout-us",
            ],
            ForkProfile::VideoStudio => {
                vec!["general", "camera-operator", "streamer", "video-editor"]
            }
            ForkProfile::SmartHome => vec!["general", "smarthome-agent", "robot-agent", "explorer"],
            ForkProfile::Factory => vec!["general", "robot-agent", "explorer", "scout-ru"],
            ForkProfile::Minimal => vec!["general"],
            ForkProfile::Full => vec![
                "general",
                "explorer",
                "planner",
                "implementer",
                "reviewer",
                "verifier",
                "weather",
                "soil",
                "drone",
                "camera-operator",
                "streamer",
                "robot-agent",
                "smarthome-agent",
            ],
        }
    }
}

pub struct ForkManager {
    pub current: ForkProfile,
    pub workspace: PathBuf,
}

impl ForkManager {
    pub fn new(profile: ForkProfile) -> Self {
        ForkManager {
            current: profile,
            workspace: PathBuf::from("."),
        }
    }

    pub fn create_fork(&self, profile: &ForkProfile) -> Result<String, String> {
        info!("ForkManager: creating fork '{}'", profile.name());
        let repo_name = profile.name();
        let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
        if token.is_empty() {
            return Err("GITHUB_TOKEN не задан. export GITHUB_TOKEN=ghp_xxx".into());
        }

        let org = GITHUB_ORG.trim_start_matches("github.com/");
        let api_url = format!("https://api.github.com/orgs/{}/repos", org);
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "name": repo_name,
            "description": profile.description(),
            "private": false,
            "auto_init": true,
        });

        match client.post(&api_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "waters-node/0.5")
            .json(&body)
            .send()
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() || status.as_u16() == 422 {
                    let skills = profile.included_skills();
                    Ok(format!("✅ Форк создан: {}/{}\n  Skills: {} | Shared: {} | Unique: {}",
                        GITHUB_ORG, repo_name, skills.len(),
                        profile.compatibility().shared_features.len(),
                        profile.compatibility().unique_features.len()))
                } else {
                    let text = resp.text().unwrap_or_default();
                    Err(format!("GitHub API error {}: {}", status, text.chars().take(200).collect::<String>()))
                }
            }
            Err(e) => Err(format!("GitHub connection failed: {}", e)),
        }
    }

        // Создать репозиторий через GitHub API
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "name": repo_name,
            "description": profile.description(),
            "private": false,
            "auto_init": true,
        });

        match client
            .post("https://api.github.com/user/repos")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "waters-node/0.5")
            .json(&body)
            .send()
        {
            Ok(resp) => {
                if resp.status().is_success() || resp.status().as_u16() == 422 {
                    // 422 = already exists, that's OK
                    let skills = profile.included_skills();
                    Ok(format!("✅ Форк '{}' готов: github.com/waters-ai/{}\n  Skills: {} | Совместимых фич: {} | 🎯 Уникальных: {}",
                        repo_name, repo_name, skills.len(),
                        profile.compatibility().shared_features.len(),
                        profile.compatibility().unique_features.len()))
                } else {
                    let status = resp.status();
                    let text = resp.text().unwrap_or_default();
                    Err(format!(
                        "GitHub API error {}: {}",
                        status,
                        text.chars().take(200).collect::<String>()
                    ))
                }
            }
            Err(e) => Err(format!("GitHub connection failed: {}", e)),
        }
    }

    /// Анализ: что из этого форка идёт в общий релиз
    pub fn analyze_common_release(&self) -> String {
        let mut out = format!(
            "📊 Анализ общего релиза для форка '{}':\n\n",
            self.current.name()
        );
        let compat = self.current.compatibility();

        out.push_str("✅ Идут в общий релиз (совместимы со всеми):\n");
        for f in &compat.shared_features {
            out.push_str(&format!("  • {}\n", f));
        }

        out.push_str("\n❌ Остаются в форке (несовместимы):\n");
        for f in &compat.unique_features {
            out.push_str(&format!("  • {}\n", f));
        }

        if !compat.incompatible_with.is_empty() {
            out.push_str("\n⚠️ Несовместимые форки:\n");
            for f in &compat.incompatible_with {
                out.push_str(&format!("  • {} — конфликт по фичам\n", f.name()));
            }
        }

        out.push_str(&format!(
            "\n📦 Итого: {} фич → общий релиз, {} → остаются в форке\n",
            compat.shared_features.len(),
            compat.unique_features.len()
        ));
        out
    }

    /// Предложить версию общего релиза
    pub fn propose_release(&self) -> String {
        let compat = self.current.compatibility();
        let ver = if compat.shared_features.len() > 5 {
            "minor"
        } else {
            "patch"
        };
        format!(
            "🚀 Предложение релиза v0.5.{}-{}\n\
             📌 Источник: {}\n\
             📦 {} shared фич → общий релиз\n\
             🎯 {} unique фич → остаются в форке\n\
             🔖 Тип: {}",
            match ver {
                "minor" => "1",
                _ => "0",
            },
            self.current.name().replace("waters-node-", ""),
            self.current.name(),
            compat.shared_features.len(),
            compat.unique_features.len(),
            if ver == "minor" {
                "minor (новые фичи)"
            } else {
                "patch (исправления)"
            }
        )
    }

    pub fn list_forks() -> Vec<ForkProfile> {
        vec![
            ForkProfile::Agriculture,
            ForkProfile::VideoStudio,
            ForkProfile::SmartHome,
            ForkProfile::Factory,
            ForkProfile::Minimal,
        ]
    }

    pub fn summary(&self) -> String {
        let mut out = format!("🍴 Текущий форк: {}\n", self.current.name());
        out.push_str(&format!("   📝 {}\n", self.current.description()));
        out.push_str(&format!("   📊 Анализ: /self release\n\n"));
        out.push_str("📦 Доступные форки:\n");
        for fork in Self::list_forks() {
            let c = fork.compatibility();
            out.push_str(&format!(
                "  {} — {} (shared:{}, unique:{})\n",
                fork.name(),
                fork.description(),
                c.shared_features.len(),
                c.unique_features.len()
            ));
        }
        out
    }
}
