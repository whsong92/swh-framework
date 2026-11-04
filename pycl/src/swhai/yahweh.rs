use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::bridge::LbrainBridge;

/// [YAHWEH/Clojure] swhAI 기호 추론 및 불변 공리계 엔진 제어기
pub struct Yahweh {
    bridge: Arc<RwLock<LbrainBridge>>,
}

impl Yahweh {
    /// YAHWEH 서브시스템 초기화 및 런타임 부트스트랩
    /// - dev-jvm: GraalVM JDK JNI를 통해 lbrain.core를 로드하고 인터프리팅/REPL 대기
    /// - release-native: GraalVM native-image로 빌드된 libyahweh.so C-ABI 직결
    pub fn init() -> Result<Self> {
        info!("[COSMOS/Rust] Initializing YAHWEH symbolic reasoning engine...");

        let bridge = LbrainBridge::init()
            .context("[YAHWEH/Clojure] Failed to bootstrap LbrainBridge")?;

        // 부트스트랩 핑 신호 전달 및 헬스체크
        let ping_msg = bridge.ping().context("[YAHWEH/Clojure] Ping verification failed")?;
        info!("[COSMOS/Rust] YAHWEH Engine Online: {}", ping_msg);

        // 핵심 진입점 부트스트랩 신호 격발 (lbrain.core/yahweh "COSMOS_BOOTSTRAP")
        let boot_sexp = "(try (lbrain.core/yahweh \"COSMOS_BOOTSTRAP\") (catch Exception e (println e)))";
        let _ = bridge.eval_sexp(boot_sexp);

        Ok(Self {
            bridge: Arc::new(RwLock::new(bridge)),
        })
    }

    /// 외부에서 초기화된 LbrainBridge 참조를 주입받는 생성자
    pub fn from_bridge(bridge: Arc<RwLock<LbrainBridge>>) -> Self {
        Self { bridge }
    }

    /// 내부 브리지 참조 획득
    pub fn bridge(&self) -> Arc<RwLock<LbrainBridge>> {
        Arc::clone(&self.bridge)
    }

    /// L0 Shared Arena 물리 메모리를 Clojure 환경에 무복사 바인딩 (DirectByteBuffer)
    pub async fn bind_shared_memory(&self, ptr: *mut f32, size: usize) -> Result<()> {
        let mut bridge = self.bridge.write().await;
        bridge.bind_shared_memory(ptr, size)?;
        info!(
            "[YAHWEH/Clojure] Bound shared physical memory at 0x{:x} ({} slots)",
            ptr as usize, size
        );
        Ok(())
    }

    /// 임의 S-expression 문자열 평가
    pub async fn eval_sexp(&self, sexp: &str) -> Result<String> {
        let bridge = self.bridge.read().await;
        bridge.eval_sexp(sexp)
    }

    // =========================================================================
    // Universal Primitives: 기호 논리 및 공리계 검증 파이프라인
    // =========================================================================

    /// 포함 (Inclusion, ⊂): 서브타입 유효성 및 개념 계층 포함 여부 판정
    pub async fn verify_inclusion(&self, sub_concept: &str, super_concept: &str) -> Result<bool> {
        let bridge = self.bridge.read().await;
        let valid = bridge.verify_inclusion(sub_concept, super_concept)?;
        
        info!(
            "[YAHWEH/Clojure] Type hierarchy inclusion check: '{}' ⊂ '{}' -> {}",
            sub_concept, super_concept, valid
        );
        Ok(valid)
    }

    /// 반대/부정 (Opposition, ¬): 공리계 모순 검증 및 상호 배타성 판정
    /// - 모순 감지 시 false를 반환하여 실행 엣지를 런타임에 절단(Pruning)하도록 유도함
    pub async fn verify_opposition(&self, traits: &[&str]) -> Result<bool> {
        let bridge = self.bridge.read().await;
        let valid = bridge.verify_opposition(traits)?;

        if !valid {
            warn!(
                "[YAHWEH/Clojure] Axiomatic contradiction detected among traits: {:?}. Causal edge candidate flagged for pruning.",
                traits
            );
        } else {
            info!(
                "[YAHWEH/Clojure] Mutual opposition/exclusion validated: {:?}",
                traits
            );
        }

        Ok(valid)
    }

    /// 동적 Clojure 스킬/공리 런타임 핫패칭 (무중단 S-식 컴파일 및 마운트)
    pub async fn register_skill(&self, skill_name: &str, code_str: &str) -> Result<bool> {
        let escaped_code = code_str.replace('\\', "\\\\").replace('"', "\\\"");
        let sexp = format!(
            "(try (lbrain.dynamic-loader/register-skill \"{}\" \"{}\") (catch Exception _ false))",
            skill_name, escaped_code
        );

        let res = self.eval_sexp(&sexp).await?;
        let success = res.trim() == "true";

        info!(
            "[YAHWEH/Clojure] Dynamic skill '{}' compilation & mounting status: {}",
            skill_name, success
        );
        Ok(success)
    }

    /// 마운트된 동적 스킬 호출 (스칼라 수치 전달)
    pub async fn call_skill(&self, skill_name: &str, arg: f32) -> Result<f32> {
        let sexp = format!(
            "(try (lbrain.dynamic-loader/call-skill \"{}\" {:.6}M) (catch Exception _ 0.0))",
            skill_name, arg
        );

        let res_str = self.eval_sexp(&sexp).await?;
        let parsed: f32 = res_str.trim().parse().unwrap_or(0.0);
        Ok(parsed)
    }

    /// L0 Shared Arena 버퍼 검증 (핑퐁 토큰 대조)
    pub async fn evaluate_buffer(&self, raw_ptr: *mut f32, byte_len: usize) -> Result<i32> {
        let sexp = format!(
            "(try (lbrain.core/evaluate-direct-buffer 0x{:x} {}) (catch Exception _ -1))",
            raw_ptr as usize, byte_len
        );

        let res_str = self.eval_sexp(&sexp).await?;
        let code: i32 = res_str.trim().parse().unwrap_or(-1);
        Ok(code)
    }
}