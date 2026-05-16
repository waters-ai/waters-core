use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::info;

/// Лёгкая бортовой Redis-подобное хранилище.
/// Если Redis недоступен — работает как in-memory HashMap (для тестов и изоляции).
/// Если Redis доступен — прозрачно использует его.

#[derive(Debug)]
pub struct KvStore {
    redis_url: Option<String>,
    redis_client: Option<redis::Client>,
    memory: Mutex<HashMap<String, String>>,
    connected: bool,
}

impl KvStore {
    pub fn new(redis_url: Option<&str>) -> Self {
        if let Some(url) = redis_url {
            if let Ok(client) = redis::Client::open(url) {
                if let Ok(mut conn) = client.get_connection() {
                    if redis::cmd("PING").query::<String>(&mut conn).is_ok() {
                        info!("KvStore connected to Redis: {}", url);
                        return KvStore {
                            redis_url: Some(url.to_string()),
                            redis_client: Some(client),
                            memory: Mutex::new(HashMap::new()),
                            connected: true,
                        };
                    }
                }
            }
            info!("KvStore Redis unavailable at {}, using in-memory", url);
        }
        KvStore { redis_url: None, redis_client: None, memory: Mutex::new(HashMap::new()), connected: false }
    }

    pub fn is_connected(&self) -> bool { self.connected }

    pub fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let _: () = redis::cmd("SETEX").arg(key).arg(ttl_secs as u64).arg(value).query(&mut conn)?;
        } else {
            self.memory.lock().unwrap().insert(key.to_string(), value.to_string());
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let val: Option<String> = redis::cmd("GET").arg(key).query(&mut conn)?;
            Ok(val)
        } else {
            Ok(self.memory.lock().unwrap().get(key).cloned())
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let _: () = redis::cmd("DEL").arg(key).query(&mut conn)?;
        } else {
            self.memory.lock().unwrap().remove(key);
        }
        Ok(())
    }

    pub fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let keys: Vec<String> = redis::cmd("KEYS").arg(format!("{}*", prefix)).query(&mut conn)?;
            Ok(keys)
        } else {
            let mem = self.memory.lock().unwrap();
            Ok(mem.keys().filter(|k| k.starts_with(prefix)).cloned().collect())
        }
    }

    /// Append to a list (journal-friendly)
    pub fn list_append(&self, key: &str, value: &str, max_len: usize) -> Result<()> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let _: () = redis::cmd("LPUSH").arg(key).arg(value).query(&mut conn)?;
            let _: () = redis::cmd("LTRIM").arg(key).arg(0).arg(max_len as isize - 1).query(&mut conn)?;
        } else {
            let mut mem = self.memory.lock().unwrap();
            let entry = mem.entry(key.to_string()).or_insert_with(String::new);
            if !entry.is_empty() { entry.insert(0, '\n'); }
            entry.insert_str(0, value);
        }
        Ok(())
    }

    /// Read recent items from a list
    pub fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let items: Vec<String> = redis::cmd("LRANGE").arg(key).arg(start).arg(stop).query(&mut conn)?;
            Ok(items)
        } else {
            let mem = self.memory.lock().unwrap();
            if let Some(val) = mem.get(key) {
                Ok(val.lines().skip(start as usize).take((stop - start) as usize).map(String::from).collect())
            } else { Ok(vec![]) }
        }
    }

    /// Publish/subscribe for real-time events
    pub fn publish(&self, channel: &str, message: &str) -> Result<()> {
        if let Some(ref client) = self.redis_client {
            let mut conn = client.get_connection()?;
            let _: () = redis::cmd("PUBLISH").arg(channel).arg(message).query(&mut conn)?;
        }
        Ok(())
    }
}
