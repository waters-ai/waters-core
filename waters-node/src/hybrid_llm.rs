use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::bridge::BridgePool;
use crate::config::Config;
use crate::store::KvStore;

// Forward declarations for subcomponents
mod edge_engine;
mod switch_protocol;
mod prefetch_cache;
mod sync_queue;

pub struct HybridLlm {
    remote: Arc<BridgePool>,
    edge: Arc<Mutex<edge_engine::EdgeEngine>>,
    switch: Arc<switch_protocol::SwitchProtocol>,
    prefetch: Arc<prefetch_cache::PrefetchCache>,
    sync: Arc<sync_queue::SyncQueue>,
    kvstore: Arc<KvStore>,
}

impl HybridLlm {
    pub fn new(remote: Arc<BridgePool>, config: &Config, kvstore: Arc<KvStore>) -> Self {
        let edge = Arc::new(Mutex::new(edge_engine::EdgeEngine::new(config)));
        let switch = Arc::new(switch_protocol::SwitchProtocol::new());
        let prefetch = Arc::new(prefetch_cache::PrefetchCache::new(config, kvstore.clone()));
        let sync = Arc::new(sync_queue::SyncQueue::new(config, kvstore.clone()));

        Self {
            remote,
            edge,
            switch,
            prefetch,
            sync,
            kvstore,
        }
    }

    pub async fn query(&self, _prompt: &str, _level: crate::autonomy::AutonomyLevel) -> String {
        // For testing network interaction, we ignore LLM and return a stub response.
        // In a real implementation, this would use the edge/remote LLMs based on autonomy level.
        format!("HybridLlm stub response")
    }
}