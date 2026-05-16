use std::io::Write;
use std::path::PathBuf;
use tracing::info;

/// Офлайн-очередь для событий, которые не ушли из-за разрыва связи.
/// Работает как append-only JSONL-файл.
/// При восстановлении связи — flush → отправка через gossip.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OfflineEvent {
    pub id: String,
    pub event_type: String,
    pub channel: String,
    pub payload: String,
    pub created_at: String,
}

pub struct OfflineQueue {
    queue_path: PathBuf,
}

impl OfflineQueue {
    pub fn new(base_path: &std::path::Path) -> Self {
        let p = base_path.join("offline").join("queue.jsonl");
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        OfflineQueue { queue_path: p }
    }

    /// Добавить событие в очередь (append-only)
    pub fn enqueue(&self, event_type: &str, channel: &str, payload: &str) -> anyhow::Result<()> {
        let event = OfflineEvent {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.to_string(),
            channel: channel.to_string(),
            payload: payload.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let line = serde_json::to_string(&event)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.queue_path)
            .map_err(|e| anyhow::anyhow!("Failed to open offline queue: {}", e))?
            .write_fmt(format_args!("{}\n", line))?;

        info!("Offline enqueued: {} ({})", event.id, event_type);
        Ok(())
    }

    /// Прочитать все события из очереди
    pub fn read_all(&self) -> anyhow::Result<Vec<OfflineEvent>> {
        if !self.queue_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.queue_path)?;
        let events: Vec<OfflineEvent> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        Ok(events)
    }

    /// Очистить очередь (после успешной отправки)
    pub fn flush(&self) -> anyhow::Result<Vec<OfflineEvent>> {
        let events = self.read_all()?;
        if events.is_empty() {
            return Ok(events);
        }
        let backup_path = self.queue_path.with_extension("jsonl.bak");
        std::fs::copy(&self.queue_path, &backup_path).ok();
        std::fs::write(&self.queue_path, "")?;
        info!("Offline queue flushed: {} events sent", events.len());
        Ok(events)
    }

    pub fn len(&self) -> anyhow::Result<usize> {
        Ok(self.read_all()?.len())
    }
}
