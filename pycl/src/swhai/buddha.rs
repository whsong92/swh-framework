// pycl/src/swhai/buddha.rs
use crate::bridge::rbrain_bridge::RbrainBridge;
use anyhow::Result;
use std::sync::Arc;

pub struct Buddha {
    bridge: Arc<RbrainBridge>,
}

impl Buddha {
    pub fn new(bridge: Arc<RbrainBridge>) -> Self {
        Self { bridge }
    }

    pub async fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.bridge.dispatch(opcode, payload_json).await
    }

    pub async fn get_status(&self) -> Result<String> {
        self.dispatch("status", "{}").await
    }
}