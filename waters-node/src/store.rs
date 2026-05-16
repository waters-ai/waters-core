use anyhow::Result;
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::info;

pub struct KvStore {
    redis_url: Option<String>,
    redis_client: Option<redis::Client>,
    memory: Mutex<HashMap<String, String>>,
    connected: bool,
    current_db: Mutex<u8>,
}

impl std::fmt::Debug for KvStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvStore")
            .field("connected", &self.connected)
            .finish()
    }
}

const SYSTEM_DB: u8 = 0;
const CACHE_DB: u8 = 15;

impl KvStore {
    pub fn new(redis_url: Option<&str>) -> Self {
        if let Some(url) = redis_url {
            if let Ok(client) = redis::Client::open(url) {
                let mut kv = KvStore {
                    redis_url: Some(url.to_string()),
                    redis_client: Some(client),
                    memory: Mutex::new(HashMap::new()),
                    connected: false,
                    current_db: Mutex::new(SYSTEM_DB),
                };
                if let Ok(mut conn) = kv.redis_client.as_ref().unwrap().get_connection() {
                    if redis::cmd("PING").query::<String>(&mut conn).is_ok() {
                        info!("KvStore connected to Redis: {}", url);
                        kv.connected = true;
                        return kv;
                    }
                }
            }
            info!("KvStore Redis unavailable at {}, using in-memory", url);
        }
        KvStore {
            redis_url: None,
            redis_client: None,
            memory: Mutex::new(HashMap::new()),
            connected: false,
            current_db: Mutex::new(SYSTEM_DB),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn select_db(&self, db: u8) -> &Self {
        *self.current_db.lock().unwrap() = db;
        self
    }

    pub fn group_db(&self, group_id: u8) -> &Self {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            SYSTEM_DB
        };
        *self.current_db.lock().unwrap() = db;
        self
    }

    pub fn system_db(&self) -> &Self {
        *self.current_db.lock().unwrap() = SYSTEM_DB;
        self
    }

    pub fn cache_db(&self) -> &Self {
        *self.current_db.lock().unwrap() = CACHE_DB;
        self
    }

    pub fn db(&self) -> u8 {
        *self.current_db.lock().unwrap()
    }

    pub fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(value)
                .query::<()>(&mut conn)?;
        } else {
            self.memory
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let val: Option<String> = redis::cmd("GET").arg(key).query(&mut conn)?;
            Ok(val)
        } else {
            Ok(self.memory.lock().unwrap().get(key).cloned())
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            redis::cmd("DEL").arg(key).query::<()>(&mut conn)?;
        } else {
            self.memory.lock().unwrap().remove(key);
        }
        Ok(())
    }

    pub fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg(format!("{}*", prefix))
                .query(&mut conn)?;
            Ok(keys)
        } else {
            let mem = self.memory.lock().unwrap();
            Ok(mem
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        }
    }

    pub fn list_append(&self, key: &str, value: &str, max_len: usize) -> Result<()> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            redis::cmd("LPUSH")
                .arg(key)
                .arg(value)
                .query::<()>(&mut conn)?;
            redis::cmd("LTRIM")
                .arg(key)
                .arg(0)
                .arg(max_len as isize - 1)
                .query::<()>(&mut conn)?;
        } else {
            let mut mem = self.memory.lock().unwrap();
            let entry = mem.entry(key.to_string()).or_insert_with(String::new);
            if !entry.is_empty() {
                entry.insert(0, '\n');
            }
            entry.insert_str(0, value);
        }
        Ok(())
    }

    pub fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let items: Vec<String> = redis::cmd("LRANGE")
                .arg(key)
                .arg(start)
                .arg(stop)
                .query(&mut conn)?;
            Ok(items)
        } else {
            let mem = self.memory.lock().unwrap();
            if let Some(val) = mem.get(key) {
                Ok(val
                    .lines()
                    .skip(start as usize)
                    .take((stop - start) as usize)
                    .map(String::from)
                    .collect())
            } else {
                Ok(vec![])
            }
        }
    }

    pub fn publish(&self, channel: &str, message: &str) -> Result<()> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            redis::cmd("PUBLISH")
                .arg(channel)
                .arg(message)
                .query::<()>(&mut conn)?;
        }
        Ok(())
    }

    pub fn xadd(&self, stream: &str, fields: &[(&str, &str)], maxlen: usize) -> Result<String> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let mut cmd = redis::cmd("XADD");
            cmd.arg(stream).arg("MAXLEN").arg(format!("~{}", maxlen));
            for &(k, v) in fields {
                cmd.arg(k).arg(v);
            }
            let id: String = cmd.query(&mut conn)?;
            Ok(id)
        } else {
            Ok("0-0".into())
        }
    }

    pub fn xlen(&self, stream: &str) -> Result<u64> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let len: u64 = redis::cmd("XLEN").arg(stream).query(&mut conn)?;
            Ok(len)
        } else {
            Ok(0)
        }
    }

    pub fn hset(&self, key: &str, field: &str, value: &str) -> Result<()> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            redis::cmd("HSET")
                .arg(key)
                .arg(field)
                .arg(value)
                .query::<()>(&mut conn)?;
        }
        Ok(())
    }

    pub fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let val: Option<String> = redis::cmd("HGET").arg(key).arg(field).query(&mut conn)?;
            Ok(val)
        } else {
            Ok(None)
        }
    }

    pub fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        if self.connected {
            let mut conn = self.redis_client.as_ref().unwrap().get_connection()?;
            let db = *self.current_db.lock().unwrap();
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            let val: HashMap<String, String> = redis::cmd("HGETALL").arg(key).query(&mut conn)?;
            Ok(val)
        } else {
            Ok(HashMap::new())
        }
    }
}

pub struct StreamSubscriber {
    connection: Option<redis::Connection>,
    channel: String,
}

impl StreamSubscriber {
    pub fn new(kvstore: &KvStore, db: u8, channel: &str) -> Result<Self> {
        if let Some(ref client) = kvstore.redis_client {
            let mut conn = client.get_connection()?;
            if db != 0 {
                redis::cmd("SELECT").arg(db).query::<()>(&mut conn)?;
            }
            redis::cmd("SUBSCRIBE")
                .arg(channel)
                .query::<()>(&mut conn)?;
            Ok(StreamSubscriber {
                connection: Some(conn),
                channel: channel.to_string(),
            })
        } else {
            anyhow::bail!("Redis not connected for subscribe")
        }
    }

    pub fn get_message(&mut self) -> Result<Option<String>> {
        if let Some(ref mut conn) = self.connection {
            let result: Vec<String> = redis::cmd("SUBSCRIBE").arg(&self.channel).query(conn)?;
            if result.len() >= 3 && result[0] == "message" {
                Ok(Some(result[2].clone()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn set_read_timeout(&mut self, secs: Option<u64>) {
        if let Some(ref mut conn) = self.connection {
            let _ = conn.set_read_timeout(secs.map(std::time::Duration::from_secs));
        }
    }

    pub fn unsubscribe(&mut self) -> Result<()> {
        if let Some(ref mut conn) = self.connection {
            redis::cmd("UNSUBSCRIBE")
                .arg(&self.channel)
                .query::<()>(conn)?;
        }
        Ok(())
    }
}
