use anyhow::{Context, Result};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::env;
use std::ffi::CString;
use std::path::{PathBuf};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info, warn};

/// 전담 CPython 워커 스레드로 전달되는 작업 단위
pub enum PythonTask {
    BindSharedMemory {
        ptr: usize,
        size: usize,
        resp: oneshot::Sender<Result<()>>,
    },
    Ping {
        resp: oneshot::Sender<Result<String>>,
    },
    EvalInclusion {
        offset_sub: usize,
        offset_super: usize,
        dim: usize,
        resp: oneshot::Sender<Result<f32>>,
    },
    EvalOpposition {
        offset_a: usize,
        offset_b: usize,
        dim: usize,
        resp: oneshot::Sender<Result<f32>>,
    },
    EvalPrimitive {
        operator: String,
        offset_a: usize,
        offset_b: usize,
        dim: usize,
        out_offset: Option<usize>,
        resp: oneshot::Sender<Result<serde_json::Value>>,
    },
    CallDynamicSkill {
        function: String,
        arg: f32,
        resp: oneshot::Sender<Result<f32>>,
    },
    MountDynamicSkill {
        function: String,
        code: String,
        resp: oneshot::Sender<Result<()>>,
    },
}

/// [BUDDHA/Python] 신경망 및 연속 직관 엔진 브리지 (Actor 기반 전담 워커)
#[derive(Clone)]
pub struct RbrainBridge {
    tx: mpsc::Sender<PythonTask>,
}

impl RbrainBridge {
    /// CPython 전담 OS 스레드를 스폰하고 mpsc 브리지를 초기화
    pub fn init(_module_hint: &str) -> Result<Self> {
        info!("[COSMOS/Rust] Initializing BUDDHA via Dedicated OS Worker Thread...");

        let (tx, mut rx) = mpsc::channel::<PythonTask>(256);
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<()>>();

        std::thread::Builder::new()
            .name("buddha-python-worker".to_string())
            .spawn(move || {
                pyo3::prepare_freethreaded_python();

                let setup_res = Python::with_gil(|py| -> Result<()> {
                    // 1. Python sys.path 경로 탐색 및 주입
                    let sys = py.import("sys")
                        .context("Failed to import sys module in CPython worker")?;
                    let path_obj = sys.getattr("path")
                        .context("Failed to get sys.path")?;
                    let sys_path = path_obj.downcast_into::<PyList>()
                        .map_err(|e| anyhow::anyhow!("Failed to downcast sys.path: {:?}", e))?;

                    // 경로 후보군 수집: 실행 위치(cwd) 기반 절대 경로 확인
                    let mut candidate_paths: Vec<PathBuf> = Vec::new();
                    if let Ok(cwd) = env::current_dir() {
                        candidate_paths.push(cwd.join("rbrain/src"));
                        candidate_paths.push(cwd.join("src"));
                        candidate_paths.push(cwd.join("../rbrain/src"));
                        candidate_paths.push(cwd.clone());
                    }
                    candidate_paths.push(PathBuf::from("/workspace/swh-framework/rbrain/src"));
                    candidate_paths.push(PathBuf::from("rbrain/src"));
                    candidate_paths.push(PathBuf::from("../rbrain/src"));

                    let mut registered_count = 0;
                    for p in candidate_paths {
                        if p.exists() {
                            let canon = p.canonicalize().unwrap_or(p);
                            let p_str = canon.to_string_lossy().to_string();
                            if let Ok(contains) = sys_path.contains(&p_str) {
                                if !contains {
                                    sys_path.insert(0, &p_str)
                                        .map_err(|e| anyhow::anyhow!("Failed to insert sys.path: {:?}", e))?;
                                    registered_count += 1;
                                }
                            }
                        }
                    }

                    info!(
                        "[BUDDHA/Python] System path setup complete (Registered {} candidates)",
                        registered_count
                    );

                    // 2. rbrain.core 임포트 및 부트스트랩 호출
                    let core = py.import("rbrain.core")
                        .context("Failed to import 'rbrain.core'. Check if rbrain/src/rbrain/core.py exists.")?;

                    // buddha("BOOTSTRAP") 또는 bootstrap() 호출
                    if let Ok(buddha_fn) = core.getattr("buddha") {
                        let _ = buddha_fn.call1(("BOOTSTRAP",))
                            .context("Exception during buddha('BOOTSTRAP') execution")?;
                    } else if let Ok(boot_fn) = core.getattr("bootstrap") {
                        let _ = boot_fn.call0()
                            .context("Exception during bootstrap() execution")?;
                    } else {
                        warn!("[BUDDHA/Python] Neither 'buddha' nor 'bootstrap' function found in rbrain.core");
                    }

                    info!("[BUDDHA/Python] rbrain.core initialized successfully.");
                    Ok(())
                });

                match setup_res {
                    Ok(()) => {
                        let _ = init_tx.send(Ok(()));
                    }
                    Err(e) => {
                        let err_msg = format!("{:?}", e);
                        error!("[BUDDHA/Python] Worker initialization failed: {}", err_msg);
                        let _ = init_tx.send(Err(e));
                        return;
                    }
                }

                // 3. Dedicated Actor 작업 디스패치 루프
                Python::with_gil(|py| {
                    info!("[BUDDHA/Python] Dedicated Worker loop started. Ready for non-blocking dispatch.");

                    while let Some(task) = rx.blocking_recv() {
                        match task {
                            PythonTask::BindSharedMemory { ptr, size, resp } => {
                                let res = Self::handle_bind_memory(py, ptr, size);
                                let _ = resp.send(res);
                            }
                            PythonTask::Ping { resp } => {
                                let res = Self::handle_ping(py);
                                let _ = resp.send(res);
                            }
                            PythonTask::EvalInclusion { offset_sub, offset_super, dim, resp } => {
                                let res = Self::handle_eval_inclusion(py, offset_sub, offset_super, dim);
                                let _ = resp.send(res);
                            }
                            PythonTask::EvalOpposition { offset_a, offset_b, dim, resp } => {
                                let res = Self::handle_eval_opposition(py, offset_a, offset_b, dim);
                                let _ = resp.send(res);
                            }
                            PythonTask::EvalPrimitive { operator, offset_a, offset_b, dim, out_offset, resp } => {
                                let res = Self::handle_eval_primitive(py, &operator, offset_a, offset_b, dim, out_offset);
                                let _ = resp.send(res);
                            }
                            PythonTask::CallDynamicSkill { function, arg, resp } => {
                                let res = Self::handle_call_dynamic_skill(py, &function, arg);
                                let _ = resp.send(res);
                            }
                            PythonTask::MountDynamicSkill { function, code, resp } => {
                                let res = Self::handle_mount_dynamic_skill(py, &function, &code);
                                let _ = resp.send(res);
                            }
                        }
                    }

                    info!("[BUDDHA/Python] Worker channel closed. Dedicated thread shutting down.");
                });
            })
            .context("[COSMOS/Rust] Failed to spawn buddha-python-worker OS thread")?;

        // 백그라운드 스레드의 초기화 성공 여부를 동기 대기 (실패 시 즉시 에러 반환)
        init_rx.recv()
            .context("[BUDDHA/Python] Worker thread dropped init channel unexpectedly")??;

        Ok(Self { tx })
    }

    // -------------------------------------------------------------------------
    // Worker 내부 동기 실행 핸들러 (GIL 보유 상태)
    // -------------------------------------------------------------------------

    fn handle_bind_memory(py: Python, ptr: usize, size: usize) -> Result<()> {
        let mem_mod = py.import("rbrain.memory")
            .context("[BUDDHA/Python] Failed to import rbrain.memory")?;
        let bind_fn = mem_mod.getattr("bind_raw_memory")
            .context("[BUDDHA/Python] bind_raw_memory function not found in rbrain.memory")?;

        bind_fn.call1((ptr, size))
            .context("[BUDDHA/Python] Exception during bind_raw_memory execution")?;
        Ok(())
    }

    fn handle_ping(py: Python) -> Result<String> {
        let core = py.import("rbrain.core")
            .context("[BUDDHA/Python] Failed to import rbrain.core")?;

        if let Ok(buddha_fn) = core.getattr("buddha") {
            let res = buddha_fn.call1(("PING",))
                .context("[BUDDHA/Python] Exception during buddha('PING')")?;
            
            // 반환값이 문자열이면 그대로 추출, 숫자(int/float) 등 객체면 str() 변환 후 추출
            if let Ok(s) = res.extract::<String>() {
                Ok(s)
            } else {
                let str_repr = res.str()?.to_string_lossy().to_string();
                Ok(format!("BUDDHA Status: {}", str_repr))
            }
        } else if let Ok(ping_fn) = core.getattr("buddha_ping") {
            let res = ping_fn.call0()
                .context("[BUDDHA/Python] Exception during buddha_ping()")?;
            Ok(res.str()?.to_string_lossy().to_string())
        } else {
            Ok("BUDDHA CPython Online (Core Loaded)".to_string())
        }
    }

    fn handle_eval_inclusion(py: Python, offset_sub: usize, offset_super: usize, dim: usize) -> Result<f32> {
        let box_mod = py.import("rbrain.primitives.box_embed")
            .context("[BUDDHA/Python] Failed to import rbrain.primitives.box_embed")?;
        let eval_fn = box_mod.getattr("eval_inclusion_offset")
            .context("[BUDDHA/Python] eval_inclusion_offset function missing")?;

        let res = eval_fn.call1((offset_sub, offset_super, dim))
            .context("[BUDDHA/Python] Exception calculating inclusion score")?;
        Ok(res.extract::<f32>()?)
    }

    fn handle_eval_opposition(py: Python, offset_a: usize, offset_b: usize, dim: usize) -> Result<f32> {
        let ortho_mod = py.import("rbrain.primitives.orthogonal")
            .context("[BUDDHA/Python] Failed to import rbrain.primitives.orthogonal")?;
        let eval_fn = ortho_mod.getattr("eval_opposition_offset")
            .context("[BUDDHA/Python] eval_opposition_offset function missing")?;

        let res = eval_fn.call1((offset_a, offset_b, dim))
            .context("[BUDDHA/Python] Exception calculating opposition score")?;
        Ok(res.extract::<f32>()?)
    }

    fn handle_eval_primitive(
        py: Python,
        operator: &str,
        offset_a: usize,
        offset_b: usize,
        dim: usize,
        out_offset: Option<usize>,
    ) -> Result<serde_json::Value> {
        let algebra_mod = py.import("rbrain.primitives.algebra")
            .context("[BUDDHA/Python] Failed to import rbrain.primitives.algebra")?;
        let eval_fn = algebra_mod.getattr("eval_primitive_offset")
            .context("[BUDDHA/Python] eval_primitive_offset function missing")?;

        let res = eval_fn.call1((operator, offset_a, offset_b, dim, out_offset))
            .context("[BUDDHA/Python] Exception executing eval_primitive_offset")?;

        let res_str: String = res.extract()?;
        let val: serde_json::Value = serde_json::from_str(&res_str).unwrap_or(serde_json::Value::Null);
        Ok(val)
    }

    fn handle_call_dynamic_skill(py: Python, function: &str, arg: f32) -> Result<f32> {
        let skills_mod = py.import("rbrain.skills")
            .context("[BUDDHA/Python] Failed to import rbrain.skills")?;
        let fn_obj = skills_mod.getattr(function)
            .with_context(|| format!("[BUDDHA/Python] Skill function '{}' not found", function))?;

        let res = fn_obj.call1((arg,))
            .context("[BUDDHA/Python] Exception calling dynamic skill")?;
        Ok(res.extract::<f32>()?)
    }

    fn handle_mount_dynamic_skill(py: Python, function: &str, code: &str) -> Result<()> {
        let sys = py.import("sys")
            .context("[BUDDHA/Python] Failed to import sys for dynamic skill mounting")?;
        let modules: Bound<PyDict> = sys
            .getattr("modules")?
            .extract::<Bound<PyDict>>()
            .map_err(|e| anyhow::anyhow!("[BUDDHA/Python] Failed to get modules dict: {:?}", e))?;

        let skills_mod = match modules.get_item("rbrain.skills")? {
            Some(m) => m,
            None => {
                let m = PyModule::new(py, "rbrain.skills")?;
                modules.set_item("rbrain.skills", &m)?;
                m.as_any().clone()
            }
        };

        let code_c = CString::new(code)
            .context("[BUDDHA/Python] Dynamic skill code contains interior null byte")?;
        let locals = skills_mod
            .getattr("__dict__")?
            .extract::<Bound<PyDict>>()
            .map_err(|e| anyhow::anyhow!("Failed to extract module __dict__: {:?}", e))?;

        py.run(&code_c, None, Some(&locals))?;

        info!("[BUDDHA/Python] Successfully mounted dynamic skill '{}'", function);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // 외부 비동기 공개 인터페이스 (Tokio Non-blocking calls)
    // -------------------------------------------------------------------------

    pub async fn bind_shared_memory(&self, ptr: *mut f32, size: usize) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::BindSharedMemory {
                ptr: ptr as usize,
                size,
                resp: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }

    pub async fn ping(&self) -> Result<String> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::Ping { resp: resp_tx })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }

    pub async fn eval_inclusion(&self, offset_sub: usize, offset_super: usize, dim: usize) -> Result<f32> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::EvalInclusion {
                offset_sub,
                offset_super,
                dim,
                resp: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }

    pub async fn eval_opposition(&self, offset_a: usize, offset_b: usize, dim: usize) -> Result<f32> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::EvalOpposition {
                offset_a,
                offset_b,
                dim,
                resp: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }

    pub async fn eval_primitive(
        &self,
        operator: &str,
        offset_a: usize,
        offset_b: usize,
        dim: usize,
        out_offset: Option<usize>,
    ) -> Result<serde_json::Value> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::EvalPrimitive {
                operator: operator.to_string(),
                offset_a,
                offset_b,
                dim,
                out_offset,
                resp: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }

    pub async fn call_dynamic_skill(&self, function: &str, arg: f32) -> Result<f32> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::CallDynamicSkill {
                function: function.to_string(),
                arg,
                resp: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }

    pub async fn mount_dynamic_skill(&self, function: &str, code: &str) -> Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(PythonTask::MountDynamicSkill {
                function: function.to_string(),
                code: code.to_string(),
                resp: resp_tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("[BUDDHA/Python] Worker dropped task receiver"))?;

        resp_rx.await.context("[BUDDHA/Python] Oneshot dropped")?
    }
}