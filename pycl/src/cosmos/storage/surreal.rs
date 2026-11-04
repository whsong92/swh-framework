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
use tracing::info;

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

        info!(
            "[COSMOS/Rust] Connecting to SurrealDB at '{}' (Timeout: {:?}, NS: {}, DB: {})...",
            clean_endpoint, timeout_dur, ns, database
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
                    eprintln!("[COSMOS/SurrealDB] Auth failed: {}. Falling back to Mock.", e);
                    return Self::mock();
                }

                if let Err(e) = surreal.use_ns(ns).use_db(database).await {
                    eprintln!("[COSMOS/SurrealDB] Scope failed: {}. Falling back to Mock.", e);
                    return Self::mock();
                }

                info!("[COSMOS/Rust] SurrealDB 3.x Cognitive Topology connected.");
                Self {
                    db: Some(surreal),
                    mock_nodes: Arc::new(Mutex::new(HashMap::new())),
                    mock_edges: Arc::new(Mutex::new(Vec::new())),
                }
            }
            Ok(Err(e)) => {
                eprintln!("[COSMOS/SurrealDB] Connect error: {}. Falling back to Mock.", e);
                Self::mock()
            }
            Err(_) => {
                eprintln!(
                    "[COSMOS/SurrealDB] Connection timed out after {:?}. Falling back to Mock.",
                    timeout_dur
                );
                Self::mock()
            }
        }
    }

    pub fn mock() -> Self {
        Self {
            db: None,
            mock_nodes: Arc::new(Mutex::new(HashMap::new())),
            mock_edges: Arc::new(Mutex::new(Vec::new())),
        }
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