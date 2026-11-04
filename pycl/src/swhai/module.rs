use serde::{Deserialize, Serialize};

/// [swhAI] 3계층 하이브리드 엔진 디스패치 타깃 식별자
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrainTarget {
    /// BUDDHA (CPython / PyTorch): 연속적 직관 및 신경망 연산
    Python {
        /// 모듈명 (예: "rbrain.core", "rbrain.dynamic_loader")
        module: String,
        /// 함수 또는 호출 가능한 심볼명 (예: "eval_primitive", "call_numeric_skill")
        function: String,
    },

    /// YAHWEH (Clojure / GraalVM): 이산적 율법, 불변 공리계 및 기호 추론
    Clojure {
        /// 네임스페이스 (예: "lbrain.core", "lbrain.primitives.contradiction")
        namespace: String,
        /// 함수 심볼명 (예: "verify-inclusion", "verify-opposition", "eval-form")
        function: String,
    },

    /// COSMOS (Rust Host Core): L0 물리 메모리 할당 및 가속 커널
    Rust {
        /// 서브시스템 (예: "arena", "storage")
        subsystem: String,
        /// 작업명 (예: "clear", "sync")
        action: String,
    },
}

impl BrainTarget {
    /// Python 타깃 간편 생성자
    pub fn python(module: impl Into<String>, function: impl Into<String>) -> Self {
        Self::Python {
            module: module.into(),
            function: function.into(),
        }
    }

    /// Clojure 타깃 간편 생성자
    pub fn clojure(namespace: impl Into<String>, function: impl Into<String>) -> Self {
        Self::Clojure {
            namespace: namespace.into(),
            function: function.into(),
        }
    }

    /// Rust 타깃 간편 생성자
    pub fn rust(subsystem: impl Into<String>, action: impl Into<String>) -> Self {
        Self::Rust {
            subsystem: subsystem.into(),
            action: action.into(),
        }
    }

    /// 대상 서브시스템 식별 문자열 반환
    pub fn subsystem_name(&self) -> &'static str {
        match self {
            Self::Python { .. } => "BUDDHA/Python",
            Self::Clojure { .. } => "YAHWEH/Clojure",
            Self::Rust { .. } => "COSMOS/Rust",
        }
    }
}

impl std::fmt::Display for BrainTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Python { module, function } => write!(f, "Python({}::{})", module, function),
            Self::Clojure { namespace, function } => write!(f, "Clojure({}/{})", namespace, function),
            Self::Rust { subsystem, action } => write!(f, "Rust({}::{})", subsystem, action),
        }
    }
}