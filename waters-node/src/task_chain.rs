use crate::skill::SkillRegistry;
use crate::subagent::SubAgentManager;
use tracing::info;

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
        task_description: &str,
    ) -> Result<String, String> {
        info!("TaskChain '{}': starting '{}'", self.name, task_description);

        if self.parallel {
            let plan = self.run_step(&ChainStep::Plan, subagents, skill_reg, task_description).await?;

            let reviews = vec![
                self.run_step(&ChainStep::Implement, subagents, skill_reg, &plan).await,
                self.run_step(&ChainStep::Implement, subagents, skill_reg, &plan).await,
            ];

            let mut results = Vec::new();
            for r in reviews {
                if let Ok(result) = r {
                    results.push(self.run_step(&ChainStep::Review, subagents, skill_reg, &result).await);
                }
            }

            let combined = results.iter().filter_map(|r| r.as_ref().ok()).cloned().collect::<Vec<_>>().join("\n");
            Ok(self.run_step(&ChainStep::Verify, subagents, skill_reg, &combined).await.unwrap_or_else(|e| e))
        } else {
            let mut current = task_description.to_string();
            for step in &self.steps {
                current = self.run_step(step, subagents, skill_reg, &current).await?;
            }
            Ok(current)
        }
    }

    async fn run_step(
        &self,
        step: &ChainStep,
        subagents: &mut SubAgentManager,
        _skill_reg: &SkillRegistry,
        input: &str,
    ) -> Result<String, String> {
        let (skill, label) = match step {
            ChainStep::Plan => ("planner", "Планирование"),
            ChainStep::Implement => ("implementer", "Реализация"),
            ChainStep::Review => ("reviewer", "Ревью"),
            ChainStep::Verify => ("verifier", "Верификация"),
            ChainStep::Deploy => ("general", "Деплой"),
        };

        info!("TaskChain step [{}]: starting", label);
        let objective = format!("{}:\n{}", label, input);

        match subagents.agent_open(skill, skill, "auto", 0, "local", None, false).await {
            Ok(agent_id) => {
                let _ = subagents.agent_assign(&agent_id, &objective, 0).await;
                let _ = subagents.agent_send_input(&agent_id, &objective, false).await;
                match subagents.agent_close(&agent_id, 0).await {
                    Ok(result) => {
                        info!("TaskChain step [{}]: completed (findings: {})", label, result.findings_count);
                        Ok(format!("[{} done] {}: {}", label, skill, result.objective))
                    }
                    Err(e) => Err(format!("Step '{}' failed: {}", label, e)),
                }
            }
            Err(e) => Err(format!("Cannot start '{}': {}", label, e)),
        }
    }

    pub fn summary(&self) -> String {
        format!("🔗 Цепочка: Plan → Implement → Review → Verify → Deploy (parallel: {})", self.parallel)
    }
}
