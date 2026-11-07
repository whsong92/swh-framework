// pycl/src/swhai/router.rs
use crate::bridge::lbrain_bridge::LbrainBridge;
use crate::bridge::rbrain_bridge::RbrainBridge;
use crate::cosmos::storage::StorageHub;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 변증법적 합의 판정 결과 (Neural + Symbolic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialecticVerdict {
    pub agreed: bool,
    pub primitive: String,
    pub neural_confidence: f32,
    pub symbolic_passed: bool,
    pub reason: String,
}

pub struct BrainRouter {
    rbrain: Arc<RwLock<RbrainBridge>>,
    lbrain: Arc<RwLock<LbrainBridge>>,
    #[allow(dead_code)]
    storage: StorageHub,
}

impl BrainRouter {
    pub fn new(
        rbrain: Arc<RwLock<RbrainBridge>>,
        lbrain: Arc<RwLock<LbrainBridge>>,
        storage: StorageHub,
    ) -> Self {
        Self {
            rbrain,
            lbrain,
            storage,
        }
    }

    /// 인과/스킬 합의 처리: Python과 Clojure의 dispatch 결과를 동기화
    pub async fn dispatch_dialectic(
        &self,
        opcode: &str,
        payload_json: &str,
    ) -> Result<DialecticVerdict> {
        // 1. BUDDHA (Python) 비동기 디스패치
        let neural_res = {
            let r = self.rbrain.read().await;
            r.dispatch(opcode, payload_json).await.ok()
        };

        // 2. YAHWEH (Clojure) 디스패치
        let symbolic_res = {
            let l = self.lbrain.read().await;
            l.dispatch(opcode, payload_json).ok()
        };

        let agreed = neural_res.is_some() && symbolic_res.is_some();
        let reason = if agreed {
            format!("Dialectic synthesis reached for opcode '{}'", opcode)
        } else {
            format!("Partial or disagreed evaluation for opcode '{}'", opcode)
        };

        info!(
            "[ROUTER] Opcode '{}' -> Agreed: {}, Neural: {}, Symbolic: {}",
            opcode,
            agreed,
            neural_res.is_some(),
            symbolic_res.is_some()
        );

        Ok(DialecticVerdict {
            agreed,
            primitive: opcode.to_string(),
            neural_confidence: if neural_res.is_some() { 1.0 } else { 0.0 },
            symbolic_passed: symbolic_res.is_some(),
            reason,
        })
    }

    /// 동적 스킬 라우팅
    pub async fn execute_dynamic_skill(&self, skill_name: &str, arg: f64) -> Result<String> {
        let payload = json!({
            "skill_name": skill_name,
            "arg": arg
        }).to_string();

        let r = self.rbrain.read().await;
        r.dispatch("dynamic_call", &payload).await
    }

    /// 동적 스킬 마운트 (Hot-Patching)
    pub async fn mount_skill(&self, skill_name: &str, code: &str) -> Result<String> {
        let payload = json!({
            "skill_name": skill_name,
            "code": code
        }).to_string();

        let r = self.rbrain.read().await;
        r.dispatch("mount_skill", &payload).await
    }

    /// 인과 추론 단계 실행
    pub async fn execute_causal_step(&self, state: serde_json::Value, action: serde_json::Value) -> Result<String> {
        let payload = json!({
            "state": state,
            "action": action
        }).to_string();

        let l = self.lbrain.read().await;
        l.dispatch("causal_step", &payload)
    }
}