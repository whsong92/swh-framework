// pycl/src/swhai/router.rs
use anyhow::Result;
use std::sync::Arc;

use crate::bridge::lbrain_bridge::LbrainBridge;
use crate::bridge::rbrain_bridge::RbrainBridge;
use crate::cosmos::storage::StorageHub;
use crate::swhai::buddha::Buddha;
use crate::swhai::yahweh::Yahweh;

/// [swhAI] 3계층 하이브리드 인지 오케스트레이터 및 변증법적 합의 라우터
pub struct BrainRouter {
    // 내부 엔진 인스턴스는 private으로 캡슐화하여 저수준 FFI 배관 직접 노출 차단
    buddha: Buddha,
    yahweh: Yahweh,
    storage: StorageHub,
}

impl BrainRouter {
    /// 브리지 핸들과 스토리지를 주입받아 라우터 인스턴스 생성
    pub fn new(
        rbrain: Arc<RbrainBridge>,
        lbrain: Arc<LbrainBridge>,
        storage: StorageHub,
    ) -> Self {
        Self {
            buddha: Buddha::new(rbrain),
            yahweh: Yahweh::new(lbrain),
            storage,
        }
    }

    // =========================================================================
    // 1. 단일 엔진 비동기 디스패치 (공개 진입점)
    // =========================================================================

    /// BUDDHA (CPython / PyTorch) 신경망 비동기 디스패치
    #[inline]
    pub async fn buddha(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.buddha.dispatch(opcode, payload_json).await
    }

    /// YAHWEH (GraalVM / Clojure) 기호 공리계 비동기 디스패치
    #[inline]
    pub async fn yahweh(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.yahweh.dispatch(opcode, payload_json).await
    }

    /// BUDDHA 상태 질의
    #[inline]
    pub async fn buddha_status(&self) -> Result<String> {
        self.buddha.get_status().await
    }

    /// YAHWEH 상태 질의
    #[inline]
    pub async fn yahweh_status(&self) -> Result<String> {
        self.yahweh.get_status().await
    }

    // =========================================================================
    // 2. 브로드캐스트 & 핑퐁
    // =========================================================================

    /// 양측 인지 엔진에 동일 명령 동시 브로드캐스트
    pub async fn broadcast(&self, opcode: &str, payload_json: &str) -> (Result<String>, Result<String>) {
        let b = self.buddha(opcode, payload_json).await;
        let y = self.yahweh(opcode, payload_json).await;
        (b, y)
    }

    /// 양측 엔진 동시 헬스체크
    pub async fn ping_all(&self) -> (Result<String>, Result<String>) {
        let b = self.buddha_status().await;
        let y = self.yahweh_status().await;
        (b, y)
    }

    // =========================================================================
    // 3. C-ABI 전역 게이트웨이 동기 디스패치 (Reverse FFI: swh_call_*)
    // =========================================================================

    /// Clojure/Native -> Python(BUDDHA) 동기 역호출
    #[inline]
    pub fn dispatch_buddha_sync(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.buddha.dispatch_sync(opcode, payload_json)
    }

    /// Python/Native -> Clojure(YAHWEH) 동기 역호출
    #[inline]
    pub fn dispatch_yahweh_sync(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.yahweh.dispatch_sync(opcode, payload_json)
    }

    // =========================================================================
    // 4. 통합 스토리지 허브 게이트웨이
    // =========================================================================

    #[inline]
    pub fn storage(&self) -> &StorageHub {
        &self.storage
    }

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