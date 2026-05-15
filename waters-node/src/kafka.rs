#![cfg(feature = "kafka-transport")]

use anyhow::Result;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub mission_id: String,
    pub agent_id: String,
    pub timestamp: String,
    pub finding_type: String,
    pub confidence: f64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub mission_id: String,
    pub order_type: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub agent_id: String,
    pub mission_id: String,
    pub timestamp: String,
    pub status: String,
    pub uptime_secs: u64,
    pub findings_count: u64,
    pub autonomy_level: u8,
}

pub struct KafkaClient {
    producer: FutureProducer,
    consumer: StreamConsumer,
    topics: Arc<Vec<String>>,
    findings_buffer: Arc<Mutex<Vec<Finding>>>,
}

impl KafkaClient {
    pub fn new(brokers: &str, group_id: &str, topics: Vec<String>) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .set("compression.type", "snappy")
            .create()?;

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "true")
            .set("auto.commit.interval.ms", "5000")
            .set("session.timeout.ms", "30000")
            .create()?;

        Ok(KafkaClient {
            producer,
            consumer,
            topics: Arc::new(topics),
            findings_buffer: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn subscribe_orders(&self) -> Result<()> {
        let topics: Vec<&str> = self.topics.iter().map(|s| s.as_str()).collect();
        self.consumer.subscribe(&topics)?;
        info!("Subscribed to topics: {:?}", topics);
        Ok(())
    }

    pub async fn consume_order(&self) -> Result<Option<Order>> {
        match tokio::time::timeout(Duration::from_secs(1), self.consumer.recv()).await {
            Ok(Ok(msg)) => {
                if let Some(payload) = msg.payload() {
                    let order: Order = serde_json::from_slice(payload)?;
                    info!("Received order: {} ({})", order.id, order.order_type);
                    Ok(Some(order))
                } else {
                    Ok(None)
                }
            }
            Ok(Err(e)) => {
                error!("Kafka consumer error: {}", e);
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }

    pub async fn publish_heartbeat(&self, topic: &str, hb: &Heartbeat) -> Result<()> {
        let payload = serde_json::to_vec(hb)?;
        let record = FutureRecord::to(topic)
            .key(&hb.agent_id)
            .payload(&payload);

        self.producer.send(record, Duration::from_secs(3)).await
            .map_err(|(e, _)| anyhow::anyhow!("Heartbeat failed: {}", e))?;
        Ok(())
    }
}
