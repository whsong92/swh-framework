// pycl/src/swhai/yahweh.rs
use crate::bridge::lbrain_bridge::LbrainBridge;
use anyhow::Result;
use std::sync::Arc;

pub struct Yahweh {
    bridge: Arc<LbrainBridge>,
}

impl Yahweh {
    pub fn new(bridge: Arc<LbrainBridge>) -> Self {
        Self { bridge }
    }

    pub async fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.bridge.dispatch(opcode, payload_json)
    }

    pub async fn get_status(&self) -> Result<String> {
        self.dispatch("status", "{}").await
    }
}