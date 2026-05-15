#![cfg(feature = "redis-storage")]

use anyhow::Result;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

pub struct RedisStore {
    client: redis::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_id: String,
    pub node_id: String,
    pub data: String,
    pub created_at: String,
}

impl RedisStore {
    pub fn new(url: &str) -> Result<Self> {
        let client = redis::Client::open(url)?;
        info!("Redis connected: {}", url);
        Ok(RedisStore { client })
    }

    pub async fn ping(&self) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.set_ex(key, value, ttl_secs as usize).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let val: Option<String> = conn.get(key).await?;
        Ok(val)
    }

    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.publish(channel, message).await?;
        Ok(())
    }

    pub async fn subscribe(&self, channel: &str) -> Result<redis::PubSub> {
        let conn = self.client.get_async_connection().await?;
        let mut pubsub = conn.into_pubsub();
        pubsub.subscribe(channel).await?;
        Ok(pubsub)
    }

    pub async fn push_task(&self, queue: &str, task: &str) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.lpush(queue, task).await?;
        Ok(())
    }

    pub async fn pop_task(&self, queue: &str, timeout_secs: u64) -> Result<Option<String>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let result: Option<(String, String)> = conn.brpop(queue, timeout_secs as usize).await?;
        Ok(result.map(|(_, item)| item))
    }

    pub async fn save_session(&self, session: &StoredSession) -> Result<()> {
        let key = format!("session:{}", session.session_id);
        let val = serde_json::to_string(session)?;
        self.set(&key, &val, 86400).await
    }

    pub async fn load_session(&self, session_id: &str) -> Result<Option<StoredSession>> {
        let key = format!("session:{}", session_id);
        let val = self.get(&key).await?;
        match val {
            Some(v) => Ok(Some(serde_json::from_str(&v)?)),
            None => Ok(None),
        }
    }

    pub async fn save_node_state(&self, node_id: &str, state: &str) -> Result<()> {
        self.set(&format!("node:{}:state", node_id), state, 604800).await
    }

    pub async fn load_node_state(&self, node_id: &str) -> Result<Option<String>> {
        self.get(&format!("node:{}:state", node_id)).await
    }

    pub async fn log_event(&self, agent_id: &str, event: &str) -> Result<()> {
        let key = format!("log:{}", agent_id);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.lpush(&key, event).await?;
        let _: () = conn.ltrim(&key, 0, 999).await?;
        Ok(())
    }

    pub async fn recent_logs(&self, agent_id: &str, count: usize) -> Result<Vec<String>> {
        let key = format!("log:{}", agent_id);
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let logs: Vec<String> = conn.lrange(&key, 0, count as isize - 1).await?;
        Ok(logs)
    }
}
