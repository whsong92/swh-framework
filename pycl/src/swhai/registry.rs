use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::cosmos::logging::{CentralLogger, LogLevel, Subsystem};
use crate::cosmos::storage::StorageHub;
use crate::swhai::module::BrainTarget;

/// 런타임 실행 대상 식별자 (모듈 라우팅 타깃)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeTarget {
    Python { module: String, func: String },
    Clojure { namespace: String, func: String },
    Native { symbol: String },
}

/// BrainTarget을 라우팅 레지스트리용 RuntimeTarget으로 변환
impl From<BrainTarget> for RuntimeTarget {
    fn from(target: BrainTarget) -> Self {
        match target {
            BrainTarget::Python { module, function } => {
                RuntimeTarget::Python { module, func: function }
            }
            BrainTarget::Clojure { namespace, function } => {
                RuntimeTarget::Clojure { namespace, func: function }
            }
            BrainTarget::Rust { subsystem, action } => {
                RuntimeTarget::Native { symbol: format!("{}::{}", subsystem, action) }
            }
        }
    }
}

/// 등록된 동적 함수/스킬의 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMeta {
    pub name: String,
    pub target: RuntimeTarget,
    pub description: String,
    pub version: u32,
}

/// [swhAI] 동적 함수 및 라우팅 심볼 레지스트리 (Thread-Safe)
pub struct FunctionRegistry {
    entries: RwLock<HashMap<String, FunctionMeta>>,
    storage: Option<StorageHub>,
}

impl FunctionRegistry {
    /// 기본 L0 물리 메모리 검증 심볼만 등록된 레지스트리 인스턴스 생성
    pub fn new(storage: Option<StorageHub>) -> Arc<Self> {
        let registry = Arc::new(Self {
            entries: RwLock::new(HashMap::new()),
            storage,
        });

        // L0 물리 메모리 파이프라인 검증용 네이티브 심볼 등록 (인프라 검증 전용)
        registry.register_internal(FunctionMeta {
            name: "l0_verify_step".to_string(),
            target: RuntimeTarget::Native {
                symbol: "cosmos::arena::verify_token".to_string(),
            },
            description: "Default 3-way L0 shared memory pipeline verification".to_string(),
            version: 1,
        });

        registry
    }

    /// 내부 전용 등록 헬퍼 (초기화 블록용 동기 호출)
    fn register_internal(&self, meta: FunctionMeta) {
        if let Ok(mut entries) = self.entries.try_write() {
            CentralLogger::log(
                Subsystem::Cosmos,
                LogLevel::Info,
                format!("Registered system symbol: '{}' -> {:?}", meta.name, meta.target),
            );
            entries.insert(meta.name.clone(), meta);
        }
    }

    /// 동적 신규 스킬 등록 (런타임 핫패칭 및 L1 Working Memory 동기화 지원)
    pub async fn register(&self, meta: FunctionMeta) {
        CentralLogger::log(
            Subsystem::Cosmos,
            LogLevel::Info,
            format!("Dynamic symbol registered: '{}' -> {:?}", meta.name, meta.target),
        );

        // L1 Redis 캐시에 활성 라우팅 테이블 동기화
        if let Some(ref storage) = self.storage {
            if let Ok(payload) = serde_json::to_string(&meta) {
                let cache_key = format!("routing:fn:{}", meta.name);
                let _ = storage.cache_state(&cache_key, &payload, Some(3600)).await;
            }
        }

        let mut entries = self.entries.write().await;
        entries.insert(meta.name.clone(), meta);
    }

    /// 심볼명으로 메타데이터 조회
    pub async fn get(&self, name: &str) -> Option<FunctionMeta> {
        let entries = self.entries.read().await;
        entries.get(name).cloned()
    }

    /// 등록된 심볼 해제 (Pruning) 및 커널 로깅
    pub async fn unregister(&self, name: &str) -> bool {
        if let Some(ref storage) = self.storage {
            let cache_key = format!("routing:fn:{}", name);
            let _ = storage.redis.del(&cache_key).await;
        }

        let mut entries = self.entries.write().await;
        let removed = entries.remove(name).is_some();
        if removed {
            CentralLogger::log(
                Subsystem::Cosmos,
                LogLevel::Warn,
                format!("Symbol unregistered from active registry: '{}'", name),
            );
        }
        removed
    }

    /// 등록된 전체 심볼 현황 출력
    pub async fn list(&self) {
        let entries = self.entries.read().await;
        println!("--------------------------------------------------------------------------------");
        println!("  Active Dynamic Function Registry (Total Registered: {})", entries.len());
        println!("--------------------------------------------------------------------------------");
        for (name, meta) in entries.iter() {
            let target_str = match &meta.target {
                RuntimeTarget::Python { module, func } => format!("BUDDHA [{}::{}]", module, func),
                RuntimeTarget::Clojure { namespace, func } => format!("YAHWEH [{}/{}]", namespace, func),
                RuntimeTarget::Native { symbol } => format!("COSMOS [{}]", symbol),
            };
            println!("  * {:<15} | {:<30} | {}", name, target_str, meta.description);
        }
        println!("--------------------------------------------------------------------------------");
    }
}