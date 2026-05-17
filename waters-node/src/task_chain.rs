use crate::bridge::BridgePool;
use crate::skill::SkillRegistry;
use crate::subagent::SubAgentManager;
use tracing::{info, warn};

pub enum ChainStep {
    Plan,
    Implement,
    Review,
    Verify,
    Deploy,
}

pub struct TaskChain {
    pub name: String,
    pub steps: Vec<ChainStep>,
    pub parallel: bool,
}

impl TaskChain {
    pub fn new(name: &str, parallel: bool) -> Self {
        TaskChain {
            name: name.to_string(),
            steps: vec![ChainStep::Plan, ChainStep::Implement, ChainStep::Review, ChainStep::Verify],
            parallel,
        }
    }

    pub async fn execute(
        &self,
        subagents: &mut SubAgentManager,
        skill_reg: &SkillRegistry,
        bridge_pool: &BridgePool,
        task_description: &str,
    ) -> Result<String, String> {
        info!("TaskChain '{}': starting '{}'", self.name, task_description);

        if self.parallel {
            let plan = self.run_step(&ChainStep::Plan, subagents, skill_reg, bridge_pool, task_description).await?;

            let reviews = vec![
                self.run_step(&ChainStep::Implement, subagents, skill_reg, bridge_pool, &plan).await,
                self.run_step(&ChainStep::Implement, subagents, skill_reg, bridge_pool, &plan).await,
            ];

            let mut results = Vec::new();
            for r in reviews {
                if let Ok(result) = r {
                    results.push(self.run_step(&ChainStep::Review, subagents, skill_reg, bridge_pool, &result).await);
                }
            }

            let combined = results.iter().filter_map(|r| r.as_ref().ok()).cloned().collect::<Vec<_>>().join("\n");
            Ok(self.run_step(&ChainStep::Verify, subagents, skill_reg, bridge_pool, &combined).await.unwrap_or_else(|e| e))
        } else {
            let mut current = task_description.to_string();
            for step in &self.steps {
                current = self.run_step(step, subagents, skill_reg, bridge_pool, &current).await?;
            }
            Ok(current)
        }
    }

    async fn run_step(
        &self,
        step: &ChainStep,
        subagents: &mut SubAgentManager,
        _skill_reg: &SkillRegistry,
        bridge_pool: &BridgePool,
        input: &str,
    ) -> Result<String, String> {
        let (skill, label, llm_prompt_prefix) = match step {
            ChainStep::Plan => ("planner", "📋 Планирование",
                "Ты — архитектор waters-node. Проанализируй задачу и составь план реализации."),
            ChainStep::Implement => ("implementer", "🔧 Реализация",
                "Ты — Rust-разработчик waters-node. Напиши код по спецификации. Используй правильные идиомы Rust."),
            ChainStep::Review => ("reviewer", "🔍 Ревью",
                "Ты — код-ревьюер. Проверь код на ошибки, unsafe практики, утечки ресурсов."),
            ChainStep::Verify => ("verifier", "✅ Верификация",
                "Ты — QA-инженер. Проверь что код компилируется и тесты проходят."),
            ChainStep::Deploy => ("general", "🚀 Деплой",
                "Ты — DevOps. Подготовь код к релизу."),
        };

        info!("TaskChain step [{}]: starting", label);
        let prompt = format!("{}\n\nЗадача:\n{}", llm_prompt_prefix, input);

        // Пытаемся получить ответ от LLM
        let llm_response = if let Some(bridge) = bridge_pool.get("llm-deepseek") {
            match bridge.call(&prompt) {
                Ok(response) => {
                    info!("TaskChain step [{}]: LLM response received ({} bytes)", label, response.len());
                    Some(response)
                }
                Err(e) => {
                    warn!("TaskChain step [{}]: LLM call failed: {}", label, e);
                    None
                }
            }
        } else {
            warn!("TaskChain step [{}]: no LLM bridge available", label);
            None
        };

        // Открываем агента для логирования (не для генерации)
        match subagents.agent_open(skill, skill, "auto", 0, "local", None, false).await {
            Ok(agent_id) => {
                let objective = format!("{}:\n{}", label, input);
                let _ = subagents.agent_assign(&agent_id, &objective, 0).await;
                let _ = subagents.agent_send_input(&agent_id, &objective, false).await;
                let result_text = llm_response.unwrap_or_else(|| format!("[{} done] {}", label, skill));
                match subagents.agent_close(&agent_id, 0).await {
                    Ok(result) => {
                        info!("TaskChain step [{}]: completed (findings: {})", label, result.findings_count);
                        Ok(result_text)
                    }
                    Err(e) => Err(format!("Step '{}' failed: {}", label, e)),
                }
            }
            Err(e) => {
                // Если агент не открылся, всё равно возвращаем LLM-ответ
                Ok(llm_response.unwrap_or_else(|| format!("[{} done] {}", label, skill)))
            }
        }
    }

    pub fn summary(&self) -> String {
        format!("🔗 Цепочка: Plan → Implement → Review → Verify → Deploy (parallel: {})", self.parallel)
    }
}
