// pycl/src/swhai/buddha.rs
use crate::bridge::rbrain_bridge::RbrainBridge;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Buddha {
    bridge: Arc<RwLock<RbrainBridge>>,
}

impl Buddha {
    pub fn new(bridge: Arc<RwLock<RbrainBridge>>) -> Self {
        Self { bridge }
    }

    pub async fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        let b = self.bridge.read().await;
        b.dispatch(opcode, payload_json).await
    }

    pub async fn get_status(&self) -> Result<String> {
        self.dispatch("status", "{}").await
    }
}