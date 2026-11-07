// pycl/src/bridge/rbrain_bridge.rs
use anyhow::{Context, Result};
use pyo3::prelude::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use tracing::{error, info};

pub enum PythonTask {
    BindSharedMemory {
        ptr: usize,
        capacity: usize,
        resp: tokio::sync::oneshot::Sender<Result<bool>>,
    },
    Dispatch {
        opcode: String,
        payload_json: String,
        resp: tokio::sync::oneshot::Sender<Result<String>>,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct RbrainBridge {
    task_tx: Sender<PythonTask>,
}

impl RbrainBridge {
    pub fn init(python_path: &str) -> Result<Self> {
        let (task_tx, task_rx): (Sender<PythonTask>, Receiver<PythonTask>) = mpsc::channel();
        let py_path = python_path.to_string();

        thread::Builder::new()
            .name("swhai-buddha-worker".to_string())
            .spawn(move || {
                pyo3::prepare_freethreaded_python();
                Python::with_gil(|py| {
                    // 1. Python sys.path 절대 경로 및 복수 후보지 등록
                    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                    let candidate_paths = [
                        py_path.clone(),
                        current_dir.join("rbrain/src").to_string_lossy().to_string(),
                        current_dir.join("../rbrain/src").to_string_lossy().to_string(),
                        current_dir.join("rbrain").to_string_lossy().to_string(),
                        current_dir.join("../rbrain").to_string_lossy().to_string(),
                    ];

                    if let Ok(sys) = py.import("sys") {
                        if let Ok(path) = sys.getattr("path") {
                            for p in &candidate_paths {
                                let _ = path.call_method1("insert", (0, p));
                            }
                        }
                    }

                    // 2. rbrain.core 임포트 및 BuddhaCore 인스턴스 생성
                    let core_module = match py.import("rbrain.core") {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("\n[BUDDHA Worker Fatal] Failed to import 'rbrain.core'!");
                            e.print_and_set_sys_last_vars(py);
                            error!("[BUDDHA Worker] Failed to import rbrain.core: {:?}", e);
                            return;
                        }
                    };

                    let buddha_core_cls = match core_module.getattr("BuddhaCore") {
                        Ok(cls) => cls,
                        Err(e) => {
                            eprintln!("\n[BUDDHA Worker Fatal] 'BuddhaCore' class not found in rbrain.core!");
                            e.print_and_set_sys_last_vars(py);
                            error!("[BUDDHA Worker] BuddhaCore class not found: {:?}", e);
                            return;
                        }
                    };

                    let buddha_instance = match buddha_core_cls.call0() {
                        Ok(inst) => inst,
                        Err(e) => {
                            eprintln!("\n[BUDDHA Worker Fatal] Failed to instantiate BuddhaCore()!");
                            e.print_and_set_sys_last_vars(py);
                            error!("[BUDDHA Worker] Failed to instantiate BuddhaCore: {:?}", e);
                            return;
                        }
                    };

                    info!("[BUDDHA Worker] CPython dedicated worker loop initialized.");

                    // 3. 메시지 루프 수신
                    while let Ok(task) = task_rx.recv() {
                        match task {
                            PythonTask::BindSharedMemory { ptr, capacity, resp } => {
                                let res = buddha_instance
                                    .call_method1("bind_arena", (ptr, capacity))
                                    .map_err(|e| {
                                        e.print_and_set_sys_last_vars(py);
                                        anyhow::anyhow!("bind_arena python error: {:?}", e)
                                    })
                                    .and_then(|val| val.extract::<bool>().map_err(Into::into));
                                let _ = resp.send(res);
                            }
                            PythonTask::Dispatch { opcode, payload_json, resp } => {
                                let res = (|| -> Result<String> {
                                    let json_mod = py.import("json")?;
                                    let py_payload = json_mod.call_method1("loads", (&payload_json,))?;
                                    let result_dict = buddha_instance.call_method1("dispatch", (&opcode, py_payload))?;
                                    let result_str: String = json_mod.call_method1("dumps", (result_dict,))?.extract()?;
                                    Ok(result_str)
                                })();
                                let _ = resp.send(res);
                            }
                            PythonTask::Shutdown => {
                                info!("[BUDDHA Worker] Shutdown signal received.");
                                break;
                            }
                        }
                    }
                });
            })
            .context("[BUDDHA] Failed to spawn dedicated Python worker thread")?;

        Ok(Self { task_tx })
    }

    pub async fn bind_shared_memory(&self, ptr: *mut f32, capacity: usize) -> Result<bool> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        self.task_tx
            .send(PythonTask::BindSharedMemory {
                ptr: ptr as usize,
                capacity,
                resp: resp_tx,
            })
            .map_err(|_| anyhow::anyhow!("[BUDDHA] Worker thread is unreachable"))?;

        resp_rx.await.context("[BUDDHA] Response channel dropped")?
    }

    pub async fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        self.task_tx
            .send(PythonTask::Dispatch {
                opcode: opcode.to_string(),
                payload_json: payload_json.to_string(),
                resp: resp_tx,
            })
            .map_err(|_| anyhow::anyhow!("[BUDDHA] Worker thread is unreachable"))?;

        resp_rx.await.context("[BUDDHA] Response channel dropped")?
    }

    pub async fn ping(&self) -> Result<String> {
        self.dispatch("status", "{}").await
    }
}