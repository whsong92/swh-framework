// pycl/src/bridge/rbrain_bridge.rs
use anyhow::{Context, Result};
use pyo3::prelude::*;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use tracing::{info};

pub enum PythonResponse {
    Async(tokio::sync::oneshot::Sender<Result<String>>),
    Sync(SyncSender<Result<String>>),
}

pub enum PythonTask {
    BindSharedMemory {
        ptr: usize,
        capacity: usize,
        resp: tokio::sync::oneshot::Sender<Result<bool>>,
    },
    Dispatch {
        opcode: String,
        payload_json: String,
        resp: PythonResponse,
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

                let buddha_unbound: Py<PyAny> = match Python::with_gil(|py| -> Result<Py<PyAny>> {
                    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

                    // 실행 위치(루트 또는 하위 pycl)에 무관하게 rbrain/src 경로를 찾아내도록 후보 경로 구성
                    let candidate_paths = [
                        py_path.clone(),
                        current_dir.join("rbrain/src").to_string_lossy().to_string(),
                        current_dir.join("../rbrain/src").to_string_lossy().to_string(),
                        current_dir.join("rbrain").to_string_lossy().to_string(),
                        current_dir.join("../rbrain").to_string_lossy().to_string(),
                        current_dir.parent().map(|p| p.join("rbrain/src").to_string_lossy().to_string()).unwrap_or_default(),
                    ];

                    if let Ok(sys) = py.import("sys") {
                        if let Ok(path) = sys.getattr("path") {
                            for p in &candidate_paths {
                                if !p.is_empty() && std::path::Path::new(p).exists() {
                                    let _ = path.call_method1("insert", (0, p));
                                }
                            }
                        }
                    }

                    // 1. Host C-ABI Function Pointer 직접 주입 (전역 라우터 인스턴스 단일화)
                    if let Ok(yahweh_proxy_mod) = py.import("rbrain.yahweh_proxy") {
                        let call_ptr = crate::swh_call_yahweh as *const () as usize;
                        let free_ptr = crate::swh_string_free as *const () as usize;
                        if let Err(e) = yahweh_proxy_mod.call_method1("_bind_host_symbols", (call_ptr, free_ptr)) {
                            eprintln!("[BUDDHA Worker] Failed to bind host C-ABI symbols to YahwehProxy: {:?}", e);
                        } else {
                            info!("[BUDDHA Worker] Bound host C-ABI symbols to YahwehProxy successfully.");
                        }
                    }

                    // 2. BuddhaCore 클래스 인스턴스 생성
                    let core_module = py.import("rbrain.core")
                        .context("Failed to import rbrain.core. Check if Python dependencies (numpy, torch) are available.")?;
                    let buddha_core_cls = core_module.getattr("BuddhaCore")
                        .context("BuddhaCore class not found in rbrain.core")?;
                    let instance = buddha_core_cls.call0()
                        .context("Failed to instantiate BuddhaCore()")?;

                    Ok(instance.unbind())
                }) {
                    Ok(inst) => inst,
                    Err(e) => {
                        // 스레드가 조기 종료될 때 tracing 필터에 걸려 삼켜지지 않도록 stderr에 즉시 출력
                        eprintln!("\n=======================================================");
                        eprintln!("[BUDDHA Worker][FATAL] Python runtime initialization failed:");
                        eprintln!("{:?}", e);
                        Python::with_gil(|py| {
                            if let Some(err) = PyErr::take(py) {
                                err.print(py);
                            }
                        });
                        eprintln!("=======================================================\n");
                        return;
                    }
                };

                info!("[BUDDHA Worker] CPython dedicated worker loop online (GIL-released on idle).");

                while let Ok(task) = task_rx.recv() {
                    match task {
                        PythonTask::Shutdown => break,
                        PythonTask::BindSharedMemory { ptr, capacity, resp } => {
                            let res = Python::with_gil(|py| {
                                let inst = buddha_unbound.bind(py);
                                inst.call_method1("bind_arena", (ptr, capacity))
                                    .map_err(|e| anyhow::anyhow!("bind_arena error: {:?}", e))
                                    .and_then(|val| val.extract::<bool>().map_err(Into::into))
                            });
                            let _ = resp.send(res);
                        }
                        PythonTask::Dispatch { opcode, payload_json, resp } => {
                            let res = Python::with_gil(|py| -> Result<String> {
                                let inst = buddha_unbound.bind(py);
                                let json_mod = py.import("json")?;
                                let py_payload = json_mod.call_method1("loads", (&payload_json,))?;
                                let result_dict = inst.call_method1("dispatch", (&opcode, py_payload))?;
                                let result_str: String = json_mod.call_method1("dumps", (result_dict,))?.extract()?;
                                Ok(result_str)
                            });

                            match resp {
                                PythonResponse::Async(tx) => {
                                    let _ = tx.send(res);
                                }
                                PythonResponse::Sync(tx) => {
                                    let _ = tx.send(res);
                                }
                            }
                        }
                    }
                }
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

    /// [Async] Tokio 비동기 런타임에서 .await 호출
    pub async fn dispatch_async(&self, opcode: &str, payload_json: &str) -> Result<String> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        self.task_tx
            .send(PythonTask::Dispatch {
                opcode: opcode.to_string(),
                payload_json: payload_json.to_string(),
                resp: PythonResponse::Async(resp_tx),
            })
            .map_err(|_| anyhow::anyhow!("[BUDDHA] Worker thread is unreachable"))?;

        resp_rx.await.context("[BUDDHA] Async response channel dropped")?
    }

    /// [Sync] C-ABI, JNI, 동기 Rust 컨텍스트에서 블로킹 호출
    pub fn dispatch_sync(&self, opcode: &str, payload_json: &str) -> Result<String> {
        let (resp_tx, resp_rx) = mpsc::sync_channel(1);
        self.task_tx
            .send(PythonTask::Dispatch {
                opcode: opcode.to_string(),
                payload_json: payload_json.to_string(),
                resp: PythonResponse::Sync(resp_tx),
            })
            .map_err(|_| anyhow::anyhow!("[BUDDHA] Worker thread is unreachable"))?;

        resp_rx.recv().context("[BUDDHA] Sync response channel dropped")?
    }

    #[inline]
    pub async fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        self.dispatch_async(opcode, payload_json).await
    }

    pub async fn ping(&self) -> Result<String> {
        self.dispatch_async("status", "{}").await
    }
}