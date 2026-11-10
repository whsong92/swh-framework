// pycl/src/lib.rs
pub mod bridge;
pub mod core;
pub mod cosmos;
pub mod swhai;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, OnceLock};

pub use core::CosmosCore;
pub use cosmos::arena::SharedArena;
pub use cosmos::logging::{CentralLogger, LogLevel, Subsystem};
pub use cosmos::storage::StorageHub;
pub use swhai::module::BrainTarget;
pub use swhai::router::BrainRouter;
pub use swhai::value::BrainValue;

/// C-ABI 상호 호출용 전역 라우터 참조
static GLOBAL_ROUTER: OnceLock<Arc<BrainRouter>> = OnceLock::new();

/// CosmosCore 부트스트랩 시 전역 FFI 핸들 등록
pub fn set_global_router(router: Arc<BrainRouter>) {
    let _ = GLOBAL_ROUTER.set(router);
}

// =============================================================================
// C-ABI Reverse Dispatch Interfaces (Python/Clojure -> Rust Host -> 상대 엔진)
// =============================================================================

/// Python / Clojure에서 YAHWEH(Clojure) 단독 디스패치 동기 호출
#[unsafe(no_mangle)]
pub extern "C" fn swh_call_yahweh(opcode: *const c_char, payload_json: *const c_char) -> *mut c_char {
    if opcode.is_null() || payload_json.is_null() {
        return CString::new("{\"error\":\"null_pointer\"}").unwrap().into_raw();
    }

    let op = unsafe { CStr::from_ptr(opcode).to_string_lossy() };
    let payload = unsafe { CStr::from_ptr(payload_json).to_string_lossy() };

    let res = if let Some(router) = GLOBAL_ROUTER.get() {
        router.dispatch_yahweh_sync(&op, &payload)
            .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    } else {
        "{\"error\":\"global_router_not_initialized\"}".to_string()
    };

    CString::new(res).unwrap().into_raw()
}

/// Clojure / Python에서 BUDDHA(Python) 단독 디스패치 동기 호출
#[unsafe(no_mangle)]
pub extern "C" fn swh_call_buddha(opcode: *const c_char, payload_json: *const c_char) -> *mut c_char {
    if opcode.is_null() || payload_json.is_null() {
        return CString::new("{\"error\":\"null_pointer\"}").unwrap().into_raw();
    }

    let op = unsafe { CStr::from_ptr(opcode).to_string_lossy() };
    let payload = unsafe { CStr::from_ptr(payload_json).to_string_lossy() };

    let res = if let Some(router) = GLOBAL_ROUTER.get() {
        router.dispatch_buddha_sync(&op, &payload)
            .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
    } else {
        "{\"error\":\"global_router_not_initialized\"}".to_string()
    };

    CString::new(res).unwrap().into_raw()
}

/// L1 Redis 문자열 저장 C-ABI 게이트웨이 (Tokio 런타임 안전 block_in_place 적용)
#[unsafe(no_mangle)]
pub extern "C" fn swh_storage_set(key: *const c_char, val_json: *const c_char, ttl_sec: u64) -> i32 {
    if key.is_null() || val_json.is_null() {
        return -1;
    }

    let k = unsafe { CStr::from_ptr(key).to_string_lossy() };
    let v = unsafe { CStr::from_ptr(val_json).to_string_lossy() };
    let ttl = if ttl_sec > 0 { Some(ttl_sec) } else { None };

    if let Some(router) = GLOBAL_ROUTER.get() {
        let res = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(router.storage_set(&k, &v, ttl)))
        } else {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map(|rt| rt.block_on(router.storage_set(&k, &v, ttl)))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Runtime error")))
        };
        if res.is_ok() { 0 } else { -2 }
    } else {
        -3
    }
}

/// L1 Redis 문자열 조회 C-ABI 게이트웨이 (Tokio 런타임 안전 block_in_place 적용)
#[unsafe(no_mangle)]
pub extern "C" fn swh_storage_get(key: *const c_char) -> *mut c_char {
    if key.is_null() {
        return std::ptr::null_mut();
    }

    let k = unsafe { CStr::from_ptr(key).to_string_lossy() };

    if let Some(router) = GLOBAL_ROUTER.get() {
        let opt_val = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(router.storage_get(&k)).ok().flatten())
        } else {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map(|rt| rt.block_on(router.storage_get(&k)).ok().flatten())
                .unwrap_or(None)
        };

        match opt_val {
            Some(v) => CString::new(v).unwrap().into_raw(),
            None => std::ptr::null_mut(),
        }
    } else {
        std::ptr::null_mut()
    }
}

/// C-ABI 문자열 메모리 반환 함수 (Memory Leak 차단)
#[unsafe(no_mangle)]
pub extern "C" fn swh_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// 역방향 중앙 로깅 C 진입점
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