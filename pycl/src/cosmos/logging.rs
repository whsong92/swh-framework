use pyo3::prelude::*;
use std::sync::mpsc::{self, SyncSender};
use std::sync::OnceLock;
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

/// 인지 아키텍처 3계층 주체
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsystem {
    Cosmos,  // Rust Host VM
    Buddha,  // Python Neural Engine
    Yahweh,  // Clojure Symbolic Engine
    General, // 범용
}

impl Subsystem {
    pub fn prefix(&self) -> &'static str {
        match self {
            Subsystem::Cosmos => "[COSMOS/Rust]",
            Subsystem::Buddha => "[BUDDHA/Python]",
            Subsystem::Yahweh => "[YAHWEH/Clojure]",
            Subsystem::General => "[COSMOS/Core]",
        }
    }
}

struct LogEntry {
    subsystem: Subsystem,
    level: LogLevel,
    message: String,
    timestamp_millis: u128,
}

pub struct CentralLogger {
    sender: SyncSender<LogEntry>,
    _worker_handle: Option<JoinHandle<()>>,
}

static CENTRAL_LOGGER: OnceLock<CentralLogger> = OnceLock::new();

impl CentralLogger {
    /// 백그라운드 로깅 워커 스레드 초기화 (비동기 채널 버퍼 4096개)
    pub fn init() {
        CENTRAL_LOGGER.get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<LogEntry>(4096);
            let handle = thread::Builder::new()
                .name("cosmos-logger-worker".to_string())
                .spawn(move || {
                    while let Ok(entry) = rx.recv() {
                        let sec = entry.timestamp_millis / 1000;
                        let millis = entry.timestamp_millis % 1000;
                        eprintln!(
                            "{:02}.{:03} {} [{}] {}",
                            sec % 100,
                            millis,
                            entry.subsystem.prefix(),
                            entry.level.as_str(),
                            entry.message
                        );
                    }
                })
                .expect("[COSMOS/Logging] Failed to spawn logger worker thread");

            CentralLogger {
                sender: tx,
                _worker_handle: Some(handle),
            }
        });
    }

    #[inline(always)]
    pub fn log(subsystem: Subsystem, level: LogLevel, msg: String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);

        let entry = LogEntry {
            subsystem,
            level,
            message: msg,
            timestamp_millis: now,
        };

        if let Some(logger) = CENTRAL_LOGGER.get() {
            let _ = logger.sender.try_send(entry);
        } else {
            eprintln!("{} [{}] {}", subsystem.prefix(), level.as_str(), entry.message);
        }
    }

    #[inline(always)]
    pub fn send(level: LogLevel, msg: String) {
        Self::log(Subsystem::Cosmos, level, msg);
    }
}

// ==========================================
// Python Reverse-FFI (PyO3 0.23 Bound API)
// ==========================================

#[pyfunction]
pub fn log_debug(msg: String) {
    CentralLogger::log(Subsystem::Buddha, LogLevel::Debug, msg);
}

#[pyfunction]
pub fn log_info(msg: String) {
    CentralLogger::log(Subsystem::Buddha, LogLevel::Info, msg);
}

#[pyfunction]
pub fn log_warn(msg: String) {
    CentralLogger::log(Subsystem::Buddha, LogLevel::Warn, msg);
}

#[pyfunction]
pub fn log_error(msg: String) {
    CentralLogger::log(Subsystem::Buddha, LogLevel::Error, msg);
}

/// CPython 호스트 인터프리터 내부에 swh_logger 모듈 주입
pub fn register_python_logger(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let logging_mod = PyModule::new(py, "swh_logger")?;
    logging_mod.add_function(wrap_pyfunction!(log_debug, &logging_mod)?)?;
    logging_mod.add_function(wrap_pyfunction!(log_info, &logging_mod)?)?;
    logging_mod.add_function(wrap_pyfunction!(log_warn, &logging_mod)?)?;
    logging_mod.add_function(wrap_pyfunction!(log_error, &logging_mod)?)?;

    // sys.modules에 주입하여 import swh_logger 즉시 사용 가능
    let sys = py.import("sys")?;
    let modules = sys.getattr("modules")?;
    modules.set_item("swh_logger", &logging_mod)?;

    m.add_submodule(&logging_mod)?;
    Ok(())
}