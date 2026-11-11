use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

pub mod mongo;
pub mod redis;
pub mod surreal;

pub use mongo::MongoStorage;
pub use redis::RedisStorage;
pub use surreal::{ConceptNode, ConceptRecord, SurrealStorage, TopologyEdge, TopologyRecord};

#[derive(Debug, Deserialize, Clone)]
pub struct CosmosConfig {
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RuntimeConfig {
    pub name: Option<String>,
    pub version: Option<String>,
    pub log_level: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct MemoryConfig {
    pub arena_slots: Option<usize>,
    pub alignment: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StorageConfig {
    pub redis: RedisConfig,
    pub surrealdb: SurrealConfig,
    pub mongo: MongoConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub connection_timeout_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SurrealConfig {
    pub endpoint: String,
    pub user: String,
    pub pass: String,
    pub namespace: String,
    pub database: String,
    pub connection_timeout_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MongoConfig {
    pub uri: String,
    pub db_name: String,
    pub connection_timeout_ms: u64,
}

impl CosmosConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<CosmosConfig>(&content) {
                info!(
                    "[COSMOS/Rust] Successfully loaded configuration from '{}'",
                    path.as_ref().display()
                );
                return cfg;
            }
        }
        info!("[COSMOS/Rust] cosmos.toml not found or parsing failed. Using fallback defaults.");
        Self {
            runtime: RuntimeConfig::default(),
            memory: MemoryConfig {
                arena_slots: Some(4194304),
                alignment: Some(64),
            },
            storage: StorageConfig {
                redis: RedisConfig {
                    url: "redis://redis:6379".to_string(),
                    connection_timeout_ms: 2000,
                },
                surrealdb: SurrealConfig {
                    endpoint: "surrealdb:8000".to_string(),
                    user: "root".to_string(),
                    pass: "root".to_string(),
                    namespace: "swhai".to_string(),
                    database: "cognition".to_string(),
                    connection_timeout_ms: 3000,
                },
                mongo: MongoConfig {
                    uri: "mongodb://root:root@mongodb:27017".to_string(),
                    db_name: "swhai_meta".to_string(),
                    connection_timeout_ms: 2000,
                },
            },
        }
    }
}

/// L1 Working Memory(Redis) + L2 Cognitive Topology(SurrealDB) 통합 허브
#[derive(Clone)]
pub struct StorageHub {
    pub redis: Arc<RedisStorage>,
    pub surreal: Arc<SurrealStorage>,
    pub mongo: Option<Arc<MongoStorage>>,
}

impl StorageHub {
    /// core.rs의 레거시 시그니처와 호환되는 연결 진입점
    pub async fn connect(
        redis_url: &str,
        surreal_endpoint: &str,
        surreal_ns: &str,
        surreal_db: &str,
        surreal_user: &str,
        surreal_pass: &str,
        mongo_uri: Option<&str>,
    ) -> Self {
        info!("[COSMOS/Storage] Initializing Unified Storage Hub (Default Timeouts: 2000ms / 3000ms)...");

        let redis = Arc::new(RedisStorage::connect(redis_url, 2000).await);
        let surreal = Arc::new(
            SurrealStorage::connect_with_timeout(
                surreal_endpoint,
                surreal_ns,
                surreal_db,
                surreal_user,
                surreal_pass,
                3000,
            )
            .await,
        );

        let mongo = if let Some(uri) = mongo_uri {
            match MongoStorage::connect(uri, "swhai_meta", 2000).await {
                Ok(m) => Some(Arc::new(m)),
                Err(e) => {
                    eprintln!("[COSMOS/Mongo] Connection failed: {:?}. Continuing without L3.", e);
                    None
                }
            }
        } else {
            None
        };

        Self { redis, surreal, mongo }
    }

    /// cosmos.toml의 전체 StorageConfig를 받아 타임아웃까지 완전 반영하는 진입점
    pub async fn connect_from_config(cfg: &StorageConfig) -> Self {
        info!("[COSMOS/Storage] Initializing Unified Storage Hub from cosmos.toml...");

        let redis = Arc::new(
            RedisStorage::connect(&cfg.redis.url, cfg.redis.connection_timeout_ms).await,
        );

        let surreal = Arc::new(
            SurrealStorage::connect_with_timeout(
                &cfg.surrealdb.endpoint,
                &cfg.surrealdb.namespace,
                &cfg.surrealdb.database,
                &cfg.surrealdb.user,
                &cfg.surrealdb.pass,
                cfg.surrealdb.connection_timeout_ms,
            )
            .await,
        );

        let mongo = match MongoStorage::connect(&cfg.mongo.uri, &cfg.mongo.db_name, cfg.mongo.connection_timeout_ms).await {
            Ok(m) => Some(Arc::new(m)),
            Err(e) => {
                eprintln!("[COSMOS/Mongo] Config-based connection failed: {:?}. Running without L3.", e);
                None
            }
        };

        Self { redis, surreal, mongo }
    }

    pub fn mock() -> Self {
        info!("[COSMOS/Storage] Initializing In-Memory Mock Storage Hub...");
        Self {
            redis: Arc::new(RedisStorage::mock()),
            surreal: Arc::new(SurrealStorage::mock()),
            mongo: None,
        }
    }

    pub async fn cache_state(&self, key: &str, json_payload: &str, ttl_sec: Option<u64>) -> Result<()> {
        self.redis.set_str(key, json_payload, ttl_sec).await
    }

    pub async fn load_state(&self, key: &str) -> Result<Option<String>> {
        self.redis.get_str(key).await
    }

    pub async fn persist_concept(&self, node: &ConceptNode) -> Result<()> {
        self.surreal.save_concept(node).await
    }

    pub async fn persist_relation(&self, edge: &TopologyEdge) -> Result<()> {
        self.surreal.record_relation(edge).await
    }

    pub async fn query_surreal(&self, sql: &str) -> Result<String> {
        self.surreal.query_raw(sql).await
    }
}