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
                let kv = KvStore {
                    redis_url: Some(url.to_string()),
                    redis_client: Some(client),
                    memory: Mutex::new(HashMap::new()),
                    connected: false,
                    current_db: Mutex::new(SYSTEM_DB),
                };
                match kv.conn() {
                    Ok(mut conn) => {
                        if redis::cmd("PING").query::<String>(&mut conn).is_ok() {
                            info!("KvStore connected to Redis: {}", url);
                            let mut kv = kv;
                            kv.connected = true;
                            return kv;
                        }
                    }
                    Err(_) => {}
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

    /// Get Redis connection or error
    fn conn(&self) -> Result<redis::Connection> {
        self.redis_client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Redis not connected"))
            .and_then(|c| Ok(c.get_connection()?))
    }

    /// Get current db and select it on the connection
    fn select_db_on_conn(&self, conn: &mut redis::Connection) -> Result<()> {
        let db = self
            .current_db
            .lock()
            .map_err(|e| anyhow::anyhow!("Mutex poisoned: {}", e))?;
        if *db != 0 {
            redis::cmd("SELECT").arg(*db).query::<()>(conn)?;
        }
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn select_db(&self, db: u8) -> &Self {
        if let Ok(mut lock) = self.current_db.lock() {
            *lock = db;
        }
        self
    }

    pub fn group_db(&self, group_id: u8) -> &Self {
        let db = if group_id >= 1 && group_id <= 6 {
            group_id
        } else {
            SYSTEM_DB
        };
        if let Ok(mut lock) = self.current_db.lock() {
            *lock = db;
        }
        self
    }

    pub fn system_db(&self) -> &Self {
        if let Ok(mut lock) = self.current_db.lock() {
            *lock = SYSTEM_DB;
        }
        self
    }

    pub fn cache_db(&self) -> &Self {
        if let Ok(mut lock) = self.current_db.lock() {
            *lock = CACHE_DB;
        }
        self
    }

    pub fn db(&self) -> u8 {
        self.current_db.lock().map(|l| *l).unwrap_or(SYSTEM_DB)
    }

    pub fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<()> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(value)
                .query::<()>(&mut conn)?;
        } else if let Ok(mut mem) = self.memory.lock() {
            mem.insert(key.to_string(), value.to_string());
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            Ok(redis::cmd("GET").arg(key).query(&mut conn)?)
        } else {
            Ok(self.memory.lock().ok().and_then(|m| m.get(key).cloned()))
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            redis::cmd("DEL").arg(key).query::<()>(&mut conn)?;
        } else if let Ok(mut mem) = self.memory.lock() {
            mem.remove(key);
        }
        Ok(())
    }

    pub fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            Ok(redis::cmd("KEYS")
                .arg(format!("{}*", prefix))
                .query(&mut conn)?)
        } else if let Ok(mem) = self.memory.lock() {
            Ok(mem
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect())
        } else {
            Ok(vec![])
        }
    }

    pub fn list_append(&self, key: &str, value: &str, max_len: usize) -> Result<()> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            redis::cmd("LPUSH")
                .arg(key)
                .arg(value)
                .query::<()>(&mut conn)?;
            redis::cmd("LTRIM")
                .arg(key)
                .arg(0)
                .arg(max_len as isize - 1)
                .query::<()>(&mut conn)?;
        } else if let Ok(mut mem) = self.memory.lock() {
            let entry = mem.entry(key.to_string()).or_default();
            if !entry.is_empty() {
                entry.insert(0, '\n');
            }
            entry.insert_str(0, value);
        }
        Ok(())
    }

    pub fn list_range(&self, key: &str, start: isize, stop: isize) -> Result<Vec<String>> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            Ok(redis::cmd("LRANGE")
                .arg(key)
                .arg(start)
                .arg(stop)
                .query(&mut conn)?)
        } else if let Ok(mem) = self.memory.lock() {
            Ok(mem
                .get(key)
                .map(|val| {
                    val.lines()
                        .skip(start as usize)
                        .take((stop - start) as usize)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default())
        } else {
            Ok(vec![])
        }
    }

    pub fn publish(&self, channel: &str, message: &str) -> Result<()> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            redis::cmd("PUBLISH")
                .arg(channel)
                .arg(message)
                .query::<()>(&mut conn)?;
        }
        Ok(())
    }

    pub fn xadd(&self, stream: &str, fields: &[(&str, &str)], maxlen: usize) -> Result<String> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            let mut cmd = redis::cmd("XADD");
            cmd.arg(stream).arg("MAXLEN").arg(format!("~{}", maxlen));
            for &(k, v) in fields {
                cmd.arg(k).arg(v);
            }
            Ok(cmd.query(&mut conn)?)
        } else {
            Ok("0-0".into())
        }
    }

    pub fn xread_all(&self, stream: &str) -> Result<Vec<HashMap<String, String>>> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            let result: Option<HashMap<String, Vec<(String, HashMap<String, String>)>>> =
                redis::cmd("XREAD")
                    .arg(&[stream, "0"])
                    .query(&mut conn)
                    .ok();
            if let Some(map) = result {
                if let Some(entries) = map.get(stream) {
                    return Ok(entries.iter().map(|(_, f)| f.clone()).collect());
                }
            }
        }
        Ok(vec![])
    }

    pub fn xread_latest(&self, stream: &str, count: usize) -> Result<Vec<HashMap<String, String>>> {
        let all = self.xread_all(stream)?;
        let start = if all.len() > count {
            all.len() - count
        } else {
            0
        };
        Ok(all[start..].to_vec())
    }

    pub fn xlen(&self, stream: &str) -> Result<u64> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            Ok(redis::cmd("XLEN").arg(stream).query(&mut conn)?)
        } else {
            Ok(0)
        }
    }

    pub fn hset(&self, key: &str, field: &str, value: &str) -> Result<()> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
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
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            Ok(redis::cmd("HGET").arg(key).arg(field).query(&mut conn)?)
        } else {
            Ok(None)
        }
    }

    pub fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        if self.connected {
            let mut conn = self.conn()?;
            self.select_db_on_conn(&mut conn)?;
            Ok(redis::cmd("HGETALL").arg(key).query(&mut conn)?)
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
        let mut conn = kvstore.conn()?;
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
            let _ = redis::cmd("UNSUBSCRIBE")
                .arg(&self.channel)
                .query::<()>(conn);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_kv() -> KvStore {
        KvStore::new(None) // in-memory mode
    }

    #[test]
    fn test_set_get() {
        let kv = make_kv();
        assert!(kv.set("test:key", "hello", 60).is_ok());
        assert_eq!(kv.get("test:key").unwrap(), Some("hello".into()));
    }

    #[test]
    fn test_delete() {
        let kv = make_kv();
        kv.set("test:del", "toberemoved", 60).unwrap();
        kv.delete("test:del").unwrap();
        assert_eq!(kv.get("test:del").unwrap(), None);
    }

    #[test]
    fn test_list_keys() {
        let kv = make_kv();
        kv.set("list:a", "1", 60).unwrap();
        kv.set("list:b", "2", 60).unwrap();
        kv.set("other:c", "3", 60).unwrap();
        let keys = kv.list_keys("list:").unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"list:a".into()));
    }

    #[test]
    fn test_list_append_range() {
        let kv = make_kv();
        kv.list_append("mylist", "first", 10).unwrap();
        kv.list_append("mylist", "second", 10).unwrap();
        let items = kv.list_range("mylist", 0, 10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], "second");
    }

    #[test]
    fn test_select_db() {
        let kv = make_kv();
        assert_eq!(kv.db(), 0);
        kv.select_db(5);
        assert_eq!(kv.db(), 5);
        kv.system_db();
        assert_eq!(kv.db(), 0);
    }

    #[test]
    fn test_hset_hget() {
        let kv = make_kv();
        kv.hset("hash:t", "field1", "val1").unwrap();
        // in-memory hset is no-op, hget returns None
        assert_eq!(kv.hget("hash:t", "field1").unwrap(), None);
    }

    #[test]
    fn test_hgetall() {
        let kv = make_kv();
        kv.hset("hash:all", "a", "1").unwrap();
        kv.hset("hash:all", "b", "2").unwrap();
        // in-memory returns empty
        let all = kv.hgetall("hash:all").unwrap();
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_publish() {
        let kv = make_kv();
        assert!(kv.publish("test:chan", "msg").is_ok());
    }

    #[test]
    fn test_xadd() {
        let kv = make_kv();
        let id = kv.xadd("test:stream", &[("key", "val")], 100).unwrap();
        assert_eq!(id, "0-0"); // in-memory returns "0-0"
    }

    #[test]
    fn test_xread_latest() {
        let kv = make_kv();
        kv.xadd("test:latest", &[("n", "1")], 100).unwrap();
        kv.xadd("test:latest", &[("n", "2")], 100).unwrap();
        let latest = kv.xread_latest("test:latest", 1).unwrap();
        assert_eq!(latest.len(), 0); // in-memory returns empty
    }

    #[test]
    fn test_group_and_cache_db() {
        let kv = make_kv();
        kv.group_db(3);
        assert_eq!(kv.db(), 3);
        kv.group_db(99); // invalid -> SYSTEM_DB
        assert_eq!(kv.db(), 0);
        kv.cache_db();
        assert_eq!(kv.db(), 15);
    }

    #[test]
    fn test_xlen() {
        let kv = make_kv();
        assert_eq!(kv.xlen("test:xlen").unwrap(), 0);
        kv.xadd("test:xlen", &[("k", "v")], 100).unwrap();
        // In memory mode, xlen returns 0 since it uses real XLEN
    }
}
