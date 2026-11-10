// swh-framework/pycl/src/cosmos/storage/redis.rs
use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::cosmos::logging::{CentralLogger, LogLevel, Subsystem};

pub enum RedisBackend {
    Live(Mutex<ConnectionManager>),
    Mock(Mutex<HashMap<String, String>>),
}

pub struct RedisStorage {
    backend: Arc<RedisBackend>,
}

impl RedisStorage {
    pub async fn connect(redis_url: &str, timeout_ms: u64) -> Self {
        let timeout_dur = Duration::from_millis(timeout_ms);
        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            format!("Connecting to Redis at '{}' (Timeout: {:?})...", redis_url, timeout_dur),
        );

        match redis::Client::open(redis_url) {
            Ok(client) => {
                match tokio::time::timeout(timeout_dur, ConnectionManager::new(client)).await {
                    Ok(Ok(manager)) => {
                        CentralLogger::log(
                            Subsystem::Cosmos,
                            LogLevel::Info,
                            format!("Redis L1 Working Memory successfully connected ({})", redis_url),
                        );
                        Self {
                            backend: Arc::new(RedisBackend::Live(Mutex::new(manager))),
                        }
                    }
                    Ok(Err(e)) => {
                        CentralLogger::log(
                            Subsystem::Cosmos,
                            LogLevel::Error,
                            format!("Redis connection failed: {:?}. Falling back to In-Memory Mock.", e),
                        );
                        Self::mock()
                    }
                    Err(_) => {
                        CentralLogger::log(
                            Subsystem::Cosmos,
                            LogLevel::Error,
                            format!("Redis connection timed out after {:?}. Falling back to In-Memory Mock.", timeout_dur),
                        );
                        Self::mock()
                    }
                }
            }
            Err(e) => {
                CentralLogger::log(
                    Subsystem::Cosmos,
                    LogLevel::Error,
                    format!("Invalid Redis URL '{}': {:?}. Falling back to In-Memory Mock.", redis_url, e),
                );
                Self::mock()
            }
        }
    }

    pub fn mock() -> Self {
        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Warn,
            "Redis In-Memory Mock initialized (L1 state will not persist across restarts).".to_string(),
        );
        Self {
            backend: Arc::new(RedisBackend::Mock(Mutex::new(HashMap::new()))),
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self.backend.as_ref(), RedisBackend::Live(_))
    }

    pub async fn set_str(&self, key: &str, value: &str, ttl_sec: Option<u64>) -> Result<()> {
        match self.backend.as_ref() {
            RedisBackend::Live(mgr_lock) => {
                let mut conn = mgr_lock.lock().await;
                if let Some(ttl) = ttl_sec {
                    let _: () = conn
                        .set_ex(key, value, ttl)
                        .await
                        .context("[COSMOS/Redis] Failed to set_ex key")?;
                } else {
                    let _: () = conn
                        .set(key, value)
                        .await
                        .context("[COSMOS/Redis] Failed to set key")?;
                }
            }
            RedisBackend::Mock(map_lock) => {
                let mut mem = map_lock.lock().await;
                mem.insert(key.to_string(), value.to_string());
            }
        }
        Ok(())
    }

    pub async fn get_str(&self, key: &str) -> Result<Option<String>> {
        match self.backend.as_ref() {
            RedisBackend::Live(mgr_lock) => {
                let mut conn = mgr_lock.lock().await;
                let val: Option<String> = conn
                    .get(key)
                    .await
                    .context("[COSMOS/Redis] Failed to get key")?;
                Ok(val)
            }
            RedisBackend::Mock(map_lock) => {
                let mem = map_lock.lock().await;
                Ok(mem.get(key).cloned())
            }
        }
    }

    pub async fn del(&self, key: &str) -> Result<bool> {
        match self.backend.as_ref() {
            RedisBackend::Live(mgr_lock) => {
                let mut conn = mgr_lock.lock().await;
                let count: i64 = conn.del(key).await.context("[COSMOS/Redis] Failed to delete key")?;
                Ok(count > 0)
            }
            RedisBackend::Mock(map_lock) => {
                let mut mem = map_lock.lock().await;
                Ok(mem.remove(key).is_some())
            }
        }
    }
}