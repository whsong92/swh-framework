use anyhow::{bail, Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::bridge::{LbrainBridge, RbrainBridge};
use crate::cosmos::storage::{StorageHub, TopologyEdge};

/// 인지 서브시스템 디스패치 타깃
#[derive(Debug, Clone)]
pub enum BrainTarget {
    Python { module: String, function: String },
    Clojure { namespace: String, function: String },
}

/// 인지 데이터 교환 값
#[derive(Debug, Clone)]
pub enum BrainValue {
    Nil,
    Boolean(bool),
    Number(f64),
    Text(String),
    TensorSlice { offset: usize, length: usize },
}

/// 변증법적 합의 결과 구조체
#[derive(Debug, Clone)]
pub struct DialecticVerdict {
    pub agreed: bool,
    pub primitive: String,
    pub neural_confidence: f32,
    pub symbolic_passed: bool,
    pub reason: String,
}

/// swhAI 인지 라우터 (Cognitive Router)
pub struct BrainRouter {
    rbrain: Arc<RwLock<RbrainBridge>>,
    lbrain: Arc<RwLock<LbrainBridge>>,
    storage: StorageHub,
    inclusion_threshold: f32,
    opposition_threshold: f32,
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
            inclusion_threshold: 0.85,
            opposition_threshold: 0.90,
        }
    }

    // =========================================================================
    // 1. Universal Primitives: 변증법적 합의 (Dialectic Agreement)
    // =========================================================================

    /// 포함 (Inclusion, A ⊂ B) 변증법적 합의
    /// - BUDDHA: 초공간 박스 임베딩 체적 포함도 점수 (Box Volume Ratio >= 0.85)
    /// - YAHWEH: 온톨로지 DAG 타입 계층 서브타입 유효성 검증
    pub async fn judge_inclusion(
        &self,
        sub_concept: &str,
        super_concept: &str,
        offset_sub: usize,
        offset_super: usize,
        dim: usize,
    ) -> Result<DialecticVerdict> {
        info!(
            "[COSMOS/Rust] Evaluating Inclusion (⊂): '{}' ⊂ '{}'",
            sub_concept, super_concept
        );

        // 1. BUDDHA 연속 공간 직관 (Box Embedding) - 비동기 채널 디스패치
        let neural_score = {
            let r = self.rbrain.read().await;
            r.eval_inclusion(offset_sub, offset_super, dim)
                .await
                .context("[BUDDHA/Python] Failed to compute box inclusion score")?
        };

        // 2. YAHWEH 이산 공간 율법 (Type Hierarchy)
        let symbolic_valid = {
            let l = self.lbrain.read().await;
            l.verify_inclusion(sub_concept, super_concept)
                .unwrap_or(false)
        };

        let neural_passed = neural_score >= self.inclusion_threshold;
        let agreed = neural_passed && symbolic_valid;

        let verdict = DialecticVerdict {
            agreed,
            primitive: "⊂".to_string(),
            neural_confidence: neural_score,
            symbolic_passed: symbolic_valid,
            reason: format!(
                "Inclusion: NeuralScore={:.4} (thresh={:.2}), SymbolicValid={}",
                neural_score, self.inclusion_threshold, symbolic_valid
            ),
        };

        info!("[COSMOS/Rust] {}", verdict.reason);

        // 합의 도달 시 L2 지식 그래프에 토폴로지 엣지 영속화
        if agreed {
            let edge = TopologyEdge {
                from: sub_concept.to_string(),
                to: super_concept.to_string(),
                primitive: "⊂".to_string(),
                weight: neural_score,
                metadata: std::collections::HashMap::new(),
            };
            let _ = self.storage.persist_relation(&edge).await;
        }

        Ok(verdict)
    }

    /// 반대/부정 (Opposition, ¬A) 변증법적 합의
    /// - BUDDHA: 직교 사영 코사인 분리도 점수 (1 - |cos(θ)| >= 0.90)
    /// - YAHWEH: 배타 공리계(Contradiction Rules) 위반 여부 검증
    pub async fn judge_opposition(
        &self,
        concept_a: &str,
        concept_b: &str,
        offset_a: usize,
        offset_b: usize,
        dim: usize,
    ) -> Result<DialecticVerdict> {
        info!(
            "[COSMOS/Rust] Evaluating Opposition (¬): '{}' ⊥ '{}'",
            concept_a, concept_b
        );

        // 1. BUDDHA 연속 공간 직교성 평가 - 비동기 채널 디스패치
        let neural_score = {
            let r = self.rbrain.read().await;
            r.eval_opposition(offset_a, offset_b, dim)
                .await
                .context("[BUDDHA/Python] Failed to compute orthogonal opposition score")?
        };

        // 2. YAHWEH 공리계 무모순 및 상호 배타성 검증
        let symbolic_valid = {
            let l = self.lbrain.read().await;
            l.verify_opposition(&[concept_a, concept_b])
                .unwrap_or(false)
        };

        let neural_passed = neural_score >= self.opposition_threshold;
        let agreed = neural_passed && symbolic_valid;

        let verdict = DialecticVerdict {
            agreed,
            primitive: "¬".to_string(),
            neural_confidence: neural_score,
            symbolic_passed: symbolic_valid,
            reason: format!(
                "Opposition: NeuralScore={:.4} (thresh={:.2}), SymbolicValid={}",
                neural_score, self.opposition_threshold, symbolic_valid
            ),
        };

        info!("[COSMOS/Rust] {}", verdict.reason);

        if agreed {
            let edge = TopologyEdge {
                from: concept_a.to_string(),
                to: concept_b.to_string(),
                primitive: "¬".to_string(),
                weight: neural_score,
                metadata: std::collections::HashMap::new(),
            };
            let _ = self.storage.persist_relation(&edge).await;
        } else if !symbolic_valid {
            warn!(
                "[YAHWEH/Clojure] Pruning causal edge: Axiomatic contradiction detected between '{}' and '{}'",
                concept_a, concept_b
            );
        }

        Ok(verdict)
    }

    /// 사칙 대수 프리미티브 (+, -, *, /) 실행 및 L0 메모리 갱신
    pub async fn execute_algebra(
        &self,
        operator: &str,
        offset_a: usize,
        offset_b: usize,
        dim: usize,
        out_offset: Option<usize>,
    ) -> Result<serde_json::Value> {
        match operator {
            "+" | "-" | "*" | "/" => {
                let r = self.rbrain.read().await;
                r.eval_primitive(operator, offset_a, offset_b, dim, out_offset)
                    .await
            }
            _ => bail!("[COSMOS/Rust] Unsupported algebraic operator: '{}'", operator),
        }
    }

    // =========================================================================
    // 2. 동적 스킬 및 S-expression 임의 디스패치
    // =========================================================================

    pub async fn dispatch(&self, target: BrainTarget, arg: BrainValue) -> Result<BrainValue> {
        match target {
            BrainTarget::Python { module: _, function } => {
                let r = self.rbrain.read().await;
                match arg {
                    BrainValue::Number(num) => {
                        let res = r.call_dynamic_skill(&function, num as f32).await?;
                        Ok(BrainValue::Number(res as f64))
                    }
                    BrainValue::Text(code) => {
                        r.mount_dynamic_skill(&function, &code).await?;
                        Ok(BrainValue::Boolean(true))
                    }
                    _ => bail!("[COSMOS/Rust] Unsupported argument type for Python dispatch"),
                }
            }
            BrainTarget::Clojure { namespace: _, function: _ } => {
                let l = self.lbrain.read().await;
                match arg {
                    BrainValue::Text(sexp) => {
                        let res = l.eval_sexp(&sexp)?;
                        Ok(BrainValue::Text(res))
                    }
                    _ => bail!("[COSMOS/Rust] Clojure dispatch expects S-expression Text"),
                }
            }
        }
    }
}