// pycl/src/cosmos/storage/mongo.rs
use anyhow::{Context, Result};
use chrono::Utc;
use mongodb::options::ClientOptions;
use mongodb::{Client, Collection, Database};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::cosmos::logging::{CentralLogger, LogLevel, Subsystem};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedQueryLog {
    pub timestamp: String,
    pub target_engine: String,
    pub intent: String,
    pub raw_query: String,
    pub execution_time_ms: f64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedLogicLog {
    pub timestamp: String,
    pub runtime: String,
    pub module_name: String,
    pub source_code: String,
    pub language: String,
    pub synthesized_by: String,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemTelemetryLog {
    pub timestamp: String,
    pub trace_id: String,
    pub entity: String,
    pub level: String,
    pub message: String,
}

#[derive(Clone)]
pub struct MongoStorage {
    db: Database,
}

impl MongoStorage {
    pub async fn connect(uri: &str, db_name: &str, timeout_ms: u64) -> Result<Self> {
        let timeout_dur = Duration::from_millis(timeout_ms);
        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            format!("Connecting to MongoDB L3 Telemetry at '{}' (Timeout: {:?})...", uri, timeout_dur),
        );

        let mut client_options = ClientOptions::parse(uri)
            .await
            .context("[COSMOS/Mongo] Failed to parse MongoDB URI")?;
        client_options.connect_timeout = Some(timeout_dur);
        client_options.server_selection_timeout = Some(timeout_dur);

        let client = Client::with_options(client_options)
            .context("[COSMOS/Mongo] Failed to create MongoClient")?;

        let db = client.database(db_name);
        db.run_command(mongodb::bson::doc! { "ping": 1 })
            .await
            .context("[COSMOS/Mongo] MongoDB ping command failed")?;

        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            format!("MongoDB L3 Telemetry Engine successfully connected (Live: '{}', DB: '{}')", uri, db_name),
        );

        Ok(Self { db })
    }

    pub async fn log_telemetry(
        &self,
        trace_id: &str,
        entity: &str,
        level: &str,
        message: &str,
    ) -> Result<()> {
        let coll: Collection<SystemTelemetryLog> = self.db.collection("system_telemetry_logs");
        let doc = SystemTelemetryLog {
            timestamp: Utc::now().to_rfc3339(),
            trace_id: trace_id.to_string(),
            entity: entity.to_string(),
            level: level.to_string(),
            message: message.to_string(),
        };
        coll.insert_one(doc).await?;
        Ok(())
    }

    pub async fn record_generated_query(
        &self,
        target_engine: &str,
        intent: &str,
        raw_query: &str,
        exec_ms: f64,
        status: &str,
    ) -> Result<()> {
        let coll: Collection<GeneratedQueryLog> = self.db.collection("generated_queries");
        let doc = GeneratedQueryLog {
            timestamp: Utc::now().to_rfc3339(),
            target_engine: target_engine.to_string(),
            intent: intent.to_string(),
            raw_query: raw_query.to_string(),
            execution_time_ms: exec_ms,
            status: status.to_string(),
        };
        coll.insert_one(doc).await?;
        Ok(())
    }

    pub async fn archive_generated_logic(
        &self,
        runtime: &str,
        module_name: &str,
        code: &str,
        lang: &str,
        synthesized_by: &str,
    ) -> Result<()> {
        let coll: Collection<GeneratedLogicLog> = self.db.collection("generated_logics");
        let doc = GeneratedLogicLog {
            timestamp: Utc::now().to_rfc3339(),
            runtime: runtime.to_string(),
            module_name: module_name.to_string(),
            source_code: code.to_string(),
            language: lang.to_string(),
            synthesized_by: synthesized_by.to_string(),
            active: true,
        };
        coll.insert_one(doc).await?;
        Ok(())
    }
}