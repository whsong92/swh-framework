// swh-framework/pycl/src/cosmos/storage/surreal.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::cosmos::logging::{CentralLogger, LogLevel, Subsystem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptNode {
    pub id: String,
    pub name: String,
    pub category: String,
    pub vector_offset: usize,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub from: String,
    pub to: String,
    pub primitive: String,
    pub weight: f32,
    pub metadata: HashMap<String, String>,
}

pub type ConceptRecord = ConceptNode;
pub type TopologyRecord = TopologyEdge;

pub struct SurrealStorage {
    db: Option<Surreal<Client>>,
    mock_nodes: Arc<Mutex<HashMap<String, ConceptNode>>>,
    mock_edges: Arc<Mutex<Vec<TopologyEdge>>>,
}

impl SurrealStorage {
    pub async fn connect(
        endpoint: &str,
        ns: &str,
        database: &str,
        user: &str,
        pass: &str,
    ) -> Self {
        Self::connect_with_timeout(endpoint, ns, database, user, pass, 3000).await
    }

    pub async fn connect_with_timeout(
        endpoint: &str,
        ns: &str,
        database: &str,
        user: &str,
        pass: &str,
        timeout_ms: u64,
    ) -> Self {
        let timeout_dur = Duration::from_millis(timeout_ms);
        let clean_endpoint = endpoint
            .trim_start_matches("ws://")
            .trim_start_matches("http://")
            .trim_end_matches("/rpc")
            .trim_end_matches('/');

        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            format!(
                "Attempting connection to SurrealDB at '{}' (Timeout: {:?}, NS: '{}', DB: '{}')...",
                clean_endpoint, timeout_dur, ns, database
            ),
        );

        match tokio::time::timeout(timeout_dur, Surreal::new::<Ws>(clean_endpoint)).await {
            Ok(Ok(surreal)) => {
                if let Err(e) = surreal
                    .signin(Root {
                        username: user.to_string(),
                        password: pass.to_string(),
                    })
                    .await
                {
                    CentralLogger::log(
                        Subsystem::Cosmos,
                        LogLevel::Error,
                        format!("SurrealDB auth failed for user '{}': {:?}. Falling back to Mock.", user, e),
                    );
                    return Self::mock();
                }

                if let Err(e) = surreal.use_ns(ns).use_db(database).await {
                    CentralLogger::log(
                        Subsystem::Cosmos,
                        LogLevel::Error,
                        format!("SurrealDB scope failed (NS: '{}', DB: '{}'): {:?}. Falling back to Mock.", ns, database, e),
                    );
                    return Self::mock();
                }

                CentralLogger::log(
                    Subsystem::Cosmos,
                    LogLevel::Info,
                    format!("SurrealDB 3.x L2 Cognitive Topology successfully connected (Live: '{}')", clean_endpoint),
                );

                Self {
                    db: Some(surreal),
                    mock_nodes: Arc::new(Mutex::new(HashMap::new())),
                    mock_edges: Arc::new(Mutex::new(Vec::new())),
                }
            }
            Ok(Err(e)) => {
                CentralLogger::log(
                    Subsystem::Cosmos,
                    LogLevel::Error,
                    format!("SurrealDB connection error to '{}': {:?}. Falling back to Mock.", clean_endpoint, e),
                );
                Self::mock()
            }
            Err(_) => {
                CentralLogger::log(
                    Subsystem::Cosmos,
                    LogLevel::Error,
                    format!("SurrealDB connection timed out after {:?}. Falling back to Mock.", timeout_dur),
                );
                Self::mock()
            }
        }
    }

    pub fn mock() -> Self {
        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Warn,
            "SurrealDB In-Memory Mock initialized (Topology not persisted to disk).".to_string(),
        );
        Self {
            db: None,
            mock_nodes: Arc::new(Mutex::new(HashMap::new())),
            mock_edges: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn is_live(&self) -> bool {
        self.db.is_some()
    }

    pub async fn save_concept(&self, node: &ConceptNode) -> Result<()> {
        if let Some(ref db) = self.db {
            let json = serde_json::to_string(node)?;
            let sql = format!("UPSERT type::thing('concept_node', '{}') CONTENT {}", node.id, json);
            db.query(sql).await.context("[COSMOS/SurrealDB] Failed to upsert concept")?;
        } else {
            let mut map = self.mock_nodes.lock().await;
            map.insert(node.id.clone(), node.clone());
        }
        Ok(())
    }

    pub async fn record_relation(&self, edge: &TopologyEdge) -> Result<()> {
        if let Some(ref db) = self.db {
            let json = serde_json::to_string(edge)?;
            let sql = format!("CREATE topology_edge CONTENT {}", json);
            db.query(sql).await.context("[COSMOS/SurrealDB] Failed to record relation")?;
        } else {
            let mut list = self.mock_edges.lock().await;
            list.push(edge.clone());
        }
        Ok(())
    }

    pub async fn query_raw(&self, sql: &str) -> Result<String> {
        if let Some(ref db) = self.db {
            let mut res = db.query(sql).await.context("[COSMOS/SurrealDB] Query failed")?;
            let values: Vec<Value> = res.take(0usize).unwrap_or_default();
            Ok(serde_json::to_string(&values)?)
        } else {
            Ok("[]".to_string())
        }
    }
}