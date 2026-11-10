// pycl/src/swhai/router.rs
use crate::bridge::lbrain_bridge::LbrainBridge;
use crate::bridge::rbrain_bridge::RbrainBridge;
use crate::cosmos::storage::StorageHub;
use anyhow::Result;
use std::sync::Arc;

pub struct BrainRouter {
    rbrain: Arc<RbrainBridge>,
    lbrain: Arc<LbrainBridge>,
    storage: StorageHub,
}

impl BrainRouter {
    pub fn new(
        rbrain: Arc<RbrainBridge>,
        lbrain: Arc<LbrainBridge>,
        storage: StorageHub,
    ) -> Self {
        Self {
            rbrain,
            lbrain,
            storage,
        }
    }

    // =========================================================================
    // 1. BUDDHA (Python) Sync / Async Dispatch (Lock-Free)
    // =========================================================================

    #[inline]
    pub async fn dispatch_buddha_async(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.rbrain.dispatch_async(opcode, payload_json).await
    }

    #[inline]
    pub fn dispatch_buddha_sync(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.rbrain.dispatch_sync(opcode, payload_json)
    }

    #[inline]
    pub async fn dispatch_buddha(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.dispatch_buddha_async(opcode, payload_json).await
    }

    // =========================================================================
    // 2. YAHWEH (Clojure) Sync / Async Dispatch (Lock-Free)
    // =========================================================================

    #[inline]
    pub async fn dispatch_yahweh_async(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.lbrain.dispatch_async(opcode, payload_json).await
    }

    #[inline]
    pub fn dispatch_yahweh_sync(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.lbrain.dispatch_sync(opcode, payload_json)
    }

    #[inline]
    pub async fn dispatch_yahweh(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.dispatch_yahweh_async(opcode, payload_json).await
    }

    // =========================================================================
    // 3. 브로드캐스트 & 핑퐁
    // =========================================================================

    pub async fn broadcast(&self, opcode: &str, payload_json: &str) -> (Result<String>, Result<String>) {
        let b = self.dispatch_buddha_async(opcode, payload_json).await;
        let y = self.dispatch_yahweh_async(opcode, payload_json).await;
        (b, y)
    }

    pub async fn ping_all(&self) -> (Result<String>, Result<String>) {
        let b = self.dispatch_buddha_async("status", "{}").await;
        let y = self.dispatch_yahweh_async("status", "{}").await;
        (b, y)
    }

    // =========================================================================
    // 4. 스토리지 단일 게이트웨이
    // =========================================================================

    pub async fn storage_set(&self, key: &str, value_json: &str, ttl_sec: Option<u64>) -> Result<()> {
        self.storage.cache_state(key, value_json, ttl_sec).await
    }

    pub async fn storage_get(&self, key: &str) -> Result<Option<String>> {
        self.storage.load_state(key).await
    }

    pub async fn storage_del(&self, key: &str) -> Result<bool> {
        self.storage.redis.del(key).await
    }

    pub async fn storage_surreal_query(&self, sql: &str) -> Result<String> {
        self.storage.query_surreal(sql).await
    }
}