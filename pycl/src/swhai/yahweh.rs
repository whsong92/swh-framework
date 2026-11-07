// pycl/src/swhai/yahweh.rs
use crate::bridge::lbrain_bridge::LbrainBridge;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Yahweh {
    bridge: Arc<RwLock<LbrainBridge>>,
}

impl Yahweh {
    pub fn new(bridge: Arc<RwLock<LbrainBridge>>) -> Self {
        Self { bridge }
    }

    pub async fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        let b = self.bridge.read().await;
        // JNI를 통한 Clojure dispatch 단일 진입점 호출
        b.dispatch(opcode, payload_json)
    }

    pub async fn get_status(&self) -> Result<String> {
        self.dispatch("status", "{}").await
    }
}