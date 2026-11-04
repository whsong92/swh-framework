use anyhow::{Context, Result};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::Value;
use std::path::Path;
use tracing::{info, warn};

use crate::cosmos::logging::register_python_logger;

/// [BUDDHA/Python] swhAI 신경망 엔진 Rust 고수준 제어기
pub struct Buddha;

impl Buddha {
    /// CPython 런타임 초기화, 로거 바인딩, 모듈 패스 마운트 및 부트스트랩 신호 송신
    pub fn init() -> Result<Self> {
        info!("[COSMOS/Rust] Initializing BUDDHA neural subsystem...");

        pyo3::prepare_freethreaded_python();

        Python::with_gil(|py| -> PyResult<()> {
            let sys = py.import("sys")?;
            let path = sys.getattr("path")?.downcast_into::<PyList>()?;

            // 1. rbrain 소스 디렉터리 경로 등록
            let rbrain_path = Path::new("../rbrain/src")
                .canonicalize()
                .unwrap_or_else(|_| Path::new("rbrain/src").to_path_buf());
            let path_str = rbrain_path.to_string_lossy();

            if !path.contains(&path_str)? {
                path.insert(0, &path_str)?;
            }

            // 2. rbrain 가상환경 site-packages 동적 탐색 및 등록
            let venv_candidates = [
                "../rbrain/.venv/lib",
                "rbrain/.venv/lib",
                ".venv/lib",
            ];

            for base in &venv_candidates {
                if let Ok(entries) = std::fs::read_dir(base) {
                    for entry in entries.flatten() {
                        let p = entry.path().join("site-packages");
                        if p.exists() {
                            if let Ok(canon) = p.canonicalize() {
                                let sp_str = canon.to_string_lossy();
                                if !path.contains(&sp_str)? {
                                    path.append(&sp_str)?;
                                }
                            }
                        }
                    }
                }
            }

            // 3. Rust-Python 통합 센트럴 로거(swh_logger) 등록
            let modules = sys.getattr("modules")?;
            if !modules.contains("swh_logger")? {
                let root_module = PyModule::new(py, "pycl_root")?;
                register_python_logger(py, &root_module)?;
            }

            // 4. BUDDHA 정식 진입점 격발
            let rbrain = py.import("rbrain.core")?;
            let buddha_fn = rbrain.getattr("buddha")?;
            let res_code: i32 = buddha_fn.call1(("COSMOS_BOOTSTRAP",))?.extract()?;

            if res_code != 0 {
                warn!("[BUDDHA/Python] Buddha awakened with non-zero status code: {}", res_code);
            }

            Ok(())
        })
        .context("[BUDDHA/Python] CPython initialization and neural bootstrap failed")?;

        info!("[COSMOS/Rust] BUDDHA neural engine successfully awakened.");
        Ok(Self)
    }

    /// 동적 신경망 스킬 실시간 핫패칭 (무중단 샌드박스 컴파일)
    pub fn register_skill(&self, skill_name: &str, code_str: &str) -> Result<bool> {
        let success: bool = Python::with_gil(|py| -> PyResult<bool> {
            let loader = py.import("rbrain.dynamic_loader")?;
            let reg_fn = loader.getattr("register_skill")?;
            let res: bool = reg_fn.call1((skill_name, code_str))?.extract()?;
            Ok(res)
        })
        .context(format!("[BUDDHA/Python] Failed to compile dynamic skill '{}'", skill_name))?;

        info!("[BUDDHA/Python] Skill '{}' registration status: {}", skill_name, success);
        Ok(success)
    }

    /// 등록된 동적 스킬 호출
    pub fn call_skill(&self, skill_name: &str, arg: f32) -> Result<f32> {
        let ret: f32 = Python::with_gil(|py| -> PyResult<f32> {
            let loader = py.import("rbrain.dynamic_loader")?;
            let call_fn = loader.getattr("call_numeric_skill")?;
            let res: f32 = call_fn.call1((skill_name, arg))?.extract()?;
            Ok(res)
        })
        .context(format!("[BUDDHA/Python] Execution failed for skill '{}'", skill_name))?;

        Ok(ret)
    }

    /// L0 Shared Arena 물리 메모리 주소를 PyTorch 텐서로 사영 및 무결성 핑퐁 검증
    pub fn process_buffer(&self, raw_addr: usize, capacity: usize) -> Result<f32> {
        let result: f32 = Python::with_gil(|py| -> PyResult<f32> {
            let rbrain = py.import("rbrain.core")?;
            let func = rbrain.getattr("process_shared_tensor")?;
            let ret: f32 = func.call1((raw_addr, capacity))?.extract()?;
            Ok(ret)
        })
        .context("[BUDDHA/Python] Shared tensor verification failed")?;

        info!("[BUDDHA/Python] Shared buffer verified at 0x{:x} (Verification Token: {})", raw_addr, result);
        Ok(result)
    }

    /// L0 Shared Arena를 통한 6대 대수 프리미티브(+, -, *, /, ⊂, ¬) 기하학적 연산 수행
    pub fn eval_primitive(
        &self,
        primitive: &str,
        offset_a: usize,
        offset_b: usize,
        dim: usize,
        out_offset: Option<usize>,
    ) -> Result<Value> {
        Python::with_gil(|py| -> PyResult<Value> {
            let rcore = py.import("rbrain.core")?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("dim", dim)?;
            if let Some(out) = out_offset {
                kwargs.set_item("out_offset", out)?;
            }

            let res_obj = rcore.call_method(
                "eval_primitive",
                (primitive, offset_a, offset_b),
                Some(&kwargs),
            )?;

            // Python dict를 JSON으로 직렬화하여 Rust Value로 안전 변환
            let json_module = py.import("json")?;
            let json_str: String = json_module.call_method1("dumps", (res_obj,))?.extract()?;

            let parsed: Value = serde_json::from_str(&json_str)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

            Ok(parsed)
        })
        .context(format!("[BUDDHA/Python] Primitive '{}' evaluation failed", primitive))
    }
}