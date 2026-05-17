/// NodeManager — главный управляющий агент ноды
/// Координирует: безопасность, самосовершенствование, группы, задачи, LLM
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub uptime_secs: u64,
    pub active_agents: u32,
    pub pending_tasks: u32,
    pub peers_connected: u32,
    pub warnings: u32,
    pub redis_ok: bool,
    pub llm_calls_today: u32,
    pub llm_cost_today: f64,
}

pub struct NodeManager {
    name: String,
    state_path: PathBuf,
    pub metrics: NodeMetrics,
    // Ссылки на подсистемы (будут заполнены при инициализации)
    pub security_learner: Option<Arc<std::sync::Mutex<crate::security::SecurityLearner>>>,
    pub skill_evolver: Option<Arc<std::sync::Mutex<crate::skill_evolve::SkillEvolver>>>,
    pub channel_isolation: Option<Arc<std::sync::Mutex<crate::security::ChannelIsolation>>>,
    pub manager_mode: ManagerMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManagerMode {
    /// Автономный — нода сама принимает решения
    Autonomous,
    /// Ручной — ждёт команд хозяина
    Manual,
    /// Совещательный — предлагает, но ждёт подтверждения
    Advisory,
}

impl NodeManager {
    pub fn new(name: &str) -> Self {
        NodeManager {
            name: name.to_string(),
            state_path: PathBuf::from(".waters/manager.json"),
            metrics: NodeMetrics {
                uptime_secs: 0,
                active_agents: 0,
                pending_tasks: 0,
                peers_connected: 0,
                warnings: 0,
                redis_ok: false,
                llm_calls_today: 0,
                llm_cost_today: 0.0,
            },
            security_learner: None,
            skill_evolver: None,
            channel_isolation: None,
            manager_mode: ManagerMode::Autonomous,
        }
    }

    /// Получить сводку состояния ноды (для /manager status)
    pub fn status(&self) -> String {
        let mode_str = match self.manager_mode {
            ManagerMode::Autonomous => "🤖 Автономный",
            ManagerMode::Manual => "👤 Ручной",
            ManagerMode::Advisory => "💡 Совещательный",
        };
        let redis_str = if self.metrics.redis_ok { "✅" } else { "❌" };
        format!(
            "🧠 Менеджер ноды '{}' [{}]\n\n\
             📊 Метрики:\n\
               Uptime: {}ч {}м\n\
               Агентов: {} | Задач: {} | Пиров: {}\n\
               Warnings: {} | LLM вызовов: {} | Стоимость: ${:.4}\n\
               Redis: {}\n\
             \n\
             🔐 Режим: {}",
            self.name,
            mode_str,
            self.metrics.uptime_secs / 3600,
            (self.metrics.uptime_secs % 3600) / 60,
            self.metrics.active_agents,
            self.metrics.pending_tasks,
            self.metrics.peers_connected,
            self.metrics.warnings,
            self.metrics.llm_calls_today,
            self.metrics.llm_cost_today * 0.0005,
            redis_str,
            mode_str,
        )
    }

    /// Запустить полный цикл улучшения
    pub fn improve(&mut self) -> Vec<String> {
        let mut steps = Vec::new();

        // 1. Безопасность
        steps.push("🔐 Проверка безопасности...".into());
        if let Some(ref learner) = self.security_learner {
            if let Ok(learner) = learner.lock() {
                let peers = learner.get_peers();
                for (id, trust) in &peers {
                    if trust.failed_interactions > 5 {
                        steps.push(format!(
                            "  ⚠️ {}: {} неудачных попыток",
                            id, trust.failed_interactions
                        ));
                    }
                }
            }
        }

        // 2. Здоровье каналов
        steps.push("🔒 Проверка изоляции каналов...".into());
        if let Some(ref ci) = self.channel_isolation {
            if let Ok(ci) = ci.lock() {
                steps.push(format!("  📊 Каналов: {}", ci.list_channels().len()));
            }
        }

        // 3. LLM оптимизация
        steps.push("🧠 Оптимизация LLM...".into());
        steps.push(format!(
            "  📞 Вызовов сегодня: {}",
            self.metrics.llm_calls_today
        ));
        if self.metrics.llm_calls_today > 1000 {
            steps.push("  💡 Рекомендация: включить кэширование (TTL 1ч)".into());
        }

        // 4. Эволюция скилов
        steps.push("🧬 Эволюция скилов...".into());
        if let Some(ref evolver) = self.skill_evolver {
            if let Ok(evolver) = evolver.lock() {
                let history_len = evolver.get_history().len();
                steps.push(format!("  📚 Историй эволюции: {}", history_len));
            }
        }

        steps.push("\n✅ Цикл улучшения завершён.".into());
        steps
    }

    pub fn set_mode(&mut self, new_mode: ManagerMode) {
        let mode_str = format!("{:?}", &new_mode);
        self.manager_mode = new_mode;
        info!("NodeManager: mode switched to {}", mode_str);
    }
}
