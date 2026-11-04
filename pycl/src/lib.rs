pub mod bridge;
pub mod core;
pub mod cosmos;
pub mod swhai;

use std::ffi::CStr;
use std::os::raw::c_char;

pub use core::CosmosCore;
pub use cosmos::arena::SharedArena;
pub use cosmos::logging::{CentralLogger, LogLevel, Subsystem};
pub use cosmos::storage::StorageHub;
pub use swhai::router::{BrainRouter, DialecticVerdict};
pub use swhai::module::BrainTarget;
pub use swhai::value::BrainValue;

/// JNA(Clojure Dev-Mode) 및 GraalVM Native AOT(libyahweh.so) C-ABI에서 호출하는 역방향 중앙 로깅 C 진입점
#[unsafe(no_mangle)]
pub extern "C" fn swh_log_native(level: *const c_char, msg: *const c_char) {
    if level.is_null() || msg.is_null() {
        return;
    }

    unsafe {
        let level_str = CStr::from_ptr(level).to_string_lossy();
        let msg_str = CStr::from_ptr(msg).to_string_lossy();

        let log_level = match level_str.trim().to_uppercase().as_str() {
            "ERROR" => LogLevel::Error,
            "WARN" => LogLevel::Warn,
            "DEBUG" => LogLevel::Debug,
            _ => LogLevel::Info,
        };

        CentralLogger::log(Subsystem::Yahweh, log_level, msg_str.into_owned());
    }
}

/// C-ABI 헬스체크 핑 인터페이스
#[unsafe(no_mangle)]
pub extern "C" fn swh_bridge_ping() -> *const c_char {
    c"COSMOS/Rust VM Core Online".as_ptr()
}