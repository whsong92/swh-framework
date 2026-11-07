use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::bridge::{LbrainBridge, RbrainBridge};
use crate::cosmos::arena::SharedArena;
use crate::cosmos::crash_guard::init_crash_guard;
use crate::cosmos::logging::{CentralLogger, LogLevel, Subsystem};
use crate::cosmos::storage::StorageHub;
use crate::swhai::registry::FunctionRegistry;
use crate::swhai::router::BrainRouter;

/// COSMOS Host Core / pycl VM 통합 컨테이너
pub struct CosmosCore {
    pub arena: Arc<RwLock<SharedArena>>,
    pub rbrain: Arc<RwLock<RbrainBridge>>,
    pub lbrain: Arc<RwLock<LbrainBridge>>,
    pub storage: StorageHub,
    pub router: Arc<BrainRouter>,
    pub registry: Arc<FunctionRegistry>,
}

impl CosmosCore {
    /// 가상 머신 및 인지 하이브리드 엔진 전체 부트스트랩 시퀀스
    pub async fn bootstrap(_config_path: &str) -> Result<Self> {
        // 0. 하드웨어 크래시 가드 및 센트럴 로거 최우선 기동
        init_crash_guard().context("[COSMOS/Rust] Failed to install hardware crash guard")?;
        CentralLogger::init();

        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            "Starting Cosmos Core & swhAI bootstrap sequence...".to_string(),
        );

        // [설정 주입] 전달받은 config_path(cosmos.toml)를 파싱 (실패 시 기본 fallback 사용)
        let config = crate::cosmos::storage::CosmosConfig::load_or_default(_config_path);

        // 1. L0 Shared Physical Arena (4M f32 = 16MB, 64-byte SIMD 정렬)
        let arena_slots = config.memory.arena_slots.unwrap_or(4 * 1024 * 1024);
        let raw_arena = SharedArena::new(arena_slots);
        let raw_ptr = raw_arena.as_mut_ptr();

        // 2. 이종 런타임 FFI 브리지 기동 (CPython BUDDHA + GraalVM YAHWEH)
        let rbrain_bridge = RbrainBridge::init("../rbrain")
            .context("[BUDDHA/Python] Failed to initialize CPython PyO3 Dedicated Worker")?;
        let mut lbrain_bridge = LbrainBridge::init()
            .context("[YAHWEH/Clojure] Failed to initialize GraalVM bridge")?;

        // 3. L0 동일 가상 메모리 주소 무복사 브로커링 (비동기 처리)
        rbrain_bridge
            .bind_shared_memory(raw_ptr, arena_slots)
            .await
            .context("[BUDDHA/Python] Failed to bind shared memory")?;
        lbrain_bridge
            .bind_shared_memory(raw_ptr, arena_slots)
            .context("[YAHWEH/Clojure] Failed to bind shared memory")?;

        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            format!(
                "Zero-Copy address brokering complete: 0x{:x} ({} MB mapped)",
                raw_ptr as usize,
                (arena_slots * std::mem::size_of::<f32>()) / (1024 * 1024)
            ),
        );

        let arena = Arc::new(RwLock::new(raw_arena));
        let rbrain = Arc::new(RwLock::new(rbrain_bridge));
        let lbrain = Arc::new(RwLock::new(lbrain_bridge));

        // 4. L1/L2 Unified Storage Hub (Redis + SurrealDB) 초기화
        let storage = StorageHub::connect(
            "redis://redis:6379",
            "surrealdb:8000",
            "cosmos",
            "topology",
            "root",
            "root",
        )
        .await;

        // 5. 인지 엔진: 동적 함수 레지스트리 및 변증법적 합의 라우터 마운트
        let registry = FunctionRegistry::new(Some(storage.clone()));
        let router = Arc::new(BrainRouter::new(
            Arc::clone(&rbrain),
            Arc::clone(&lbrain),
            storage.clone(),
        ));

        // C-ABI Reverse Dispatch용 전역 바인딩 등록 (A 방식)
        crate::set_global_router(Arc::clone(&router));

        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            "Cosmos Core kernel successfully bootstrapped.".to_string(),
        );

        Ok(Self {
            arena,
            rbrain,
            lbrain,
            storage,
            router,
            registry,
        })
    }

    /// 인지 엔진 헬스체크 및 부트스트랩 핑퐁
    pub async fn init_cognitive_engines(&self) -> Result<()> {
        let b_status = {
            let r = self.rbrain.read().await;
            r.ping().await?
        };
        let y_status = {
            let l = self.lbrain.read().await;
            l.ping()?
        };

        CentralLogger::log(
            Subsystem::Buddha,
            LogLevel::Info,
            format!("Subsystem Online: {}", b_status),
        );
        CentralLogger::log(
            Subsystem::Yahweh,
            LogLevel::Info,
            format!("Subsystem Online: {}", y_status),
        );

        Ok(())
    }

    /// 가상 머신 안전 정지 시퀀스
    pub async fn shutdown(&self) -> Result<()> {
        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            "Releasing shared physical arena and halting pycl VM safely.".to_string(),
        );
        Ok(())
    }
}