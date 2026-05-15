use anyhow::Result;
use crate::subagent::SubAgentManager;
use crate::llm::LlmClient;
use crate::mcp::McpClient;
use std::time::Instant;
use tracing::{info, warn};

pub struct AgentEngine {
    agent_id: String,
    mission_id: String,
    subagents: SubAgentManager,
    llm: Option<LlmClient>,
    mcp: McpClient,
    findings_count: u64,

    #[cfg(feature = "kafka-transport")]
    kafka: Option<crate::kafka::KafkaClient>,
}

impl AgentEngine {
    pub fn new(agent_id: &str, mission_id: &str) -> Self {
        AgentEngine {
            agent_id: agent_id.to_string(),
            mission_id: mission_id.to_string(),
            subagents: SubAgentManager::new(),
            llm: None,
            mcp: McpClient::new(),
            findings_count: 0,

            #[cfg(feature = "kafka-transport")]
            kafka: None,
        }
    }

    pub fn agent_id(&self) -> &str { &self.agent_id }
    pub fn mission_id(&self) -> &str { &self.mission_id }
    pub fn subagents(&mut self) -> &mut SubAgentManager { &mut self.subagents }

    pub fn attach_llm(&mut self, llm: LlmClient) {
        self.llm = Some(llm);
        info!("LLM client attached");
    }

    pub fn register_mcp(&mut self, name: &str, transport: &str, command: &str, args: &[String]) {
        self.mcp.register(name, transport, command, args);
    }

    #[cfg(feature = "kafka-transport")]
    pub async fn enable_kafka(&mut self, kafka_cfg: &crate::config::KafkaConfig) -> Result<()> {
        let client = crate::kafka::KafkaClient::new(
            &kafka_cfg.brokers,
            &kafka_cfg.group_id,
            vec![kafka_cfg.topics.orders.clone(), kafka_cfg.topics.config.clone()],
        )?;
        client.subscribe_orders().await?;
        self.kafka = Some(client);
        Ok(())
    }

    pub fn kafka_enabled(&self) -> bool {
        #[cfg(feature = "kafka-transport")]
        { self.kafka.is_some() }
        #[cfg(not(feature = "kafka-transport"))]
        { false }
    }

    pub async fn tick(&mut self, autonomy: &mut crate::autonomy::AutonomyEngine, start: Instant) {
        let mut has_work = false;

        #[cfg(feature = "kafka-transport")]
        if let Some(ref kafka) = self.kafka {
            if let Ok(Some(order)) = kafka.consume_order().await {
                info!("Kafka order: {} ({})", order.id, order.order_type);
                has_work = true;
                match order.order_type.as_str() {
                    "spawn_agent" => {
                        let role = order.payload.get("role").and_then(|v| v.as_str()).unwrap_or("general");
                        let id = self.subagents.spawn(role);
                        info!("Spawned sub-agent {} from Kafka order", id);
                    }
                    "ping" => info!("Ping received"),
                    "shutdown" => {
                        info!("Shutdown received via Kafka");
                        std::process::exit(0);
                    }
                    _ => info!("Unknown order type: {}", order.order_type),
                }
            }
        }

        // Heartbeat every 30s in standalone mode
        let uptime = start.elapsed().as_secs();
        if uptime > 0 && uptime % 30 == 0 && !has_work {
            info!(
                "❤️  {} | up={}s | subagents={} | findings={} | autonomy={}",
                self.agent_id,
                uptime,
                self.subagents.count(),
                self.findings_count,
                autonomy.current(),
            );

            #[cfg(feature = "kafka-transport")]
            if let Some(ref kafka) = self.kafka {
                let hb = crate::kafka::Heartbeat {
                    agent_id: self.agent_id.clone(),
                    mission_id: self.mission_id.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    status: "alive".into(),
                    uptime_secs: uptime,
                    findings_count: self.findings_count,
                    autonomy_level: autonomy.current() as u8,
                };
                if let Err(e) = kafka.publish_heartbeat("mission.1.heartbeat.v1", &hb).await {
                    warn!("Heartbeat failed: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
