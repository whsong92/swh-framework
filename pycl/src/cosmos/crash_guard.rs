// pycl/src/cosmos/crash_guard.rs
use anyhow::{bail, Result};
use libc::{
    c_int, c_void, sigaction, siginfo_t,
    SA_NODEFER, SA_ONSTACK, SA_SIGINFO, SIGBUS, SIGFPE, SIGILL, SIGSEGV,
};
use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, warn};

#[repr(C, align(16))]
#[derive(Copy, Clone)]
pub struct JmpBuf {
    _buffer: [u8; 256],
}

// Rust 2024 규격: unsafe extern "C"
unsafe extern "C" {
    #[link_name = "__sigsetjmp"]
    fn sigsetjmp_c(env: *mut JmpBuf, savemask: c_int) -> c_int;
    #[link_name = "siglongjmp"]
    fn siglongjmp_c(env: *mut JmpBuf, val: c_int) -> !;
}

static GUARD_INSTALLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static JUMP_ENV: RefCell<Option<JmpBuf>> = const { RefCell::new(None) };
    static IN_GUARD_ZONE: RefCell<bool> = const { RefCell::new(false) };
}

#[derive(Debug, Clone)]
pub struct CrashInfo {
    pub signal: i32,
    pub signal_name: &'static str,
    pub fault_addr: usize,
}

impl std::fmt::Display for CrashInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Hardware Fault Trapped: {} (code: {}) at 0x{:x}",
            self.signal_name, self.signal, self.fault_addr
        )
    }
}

impl std::error::Error for CrashInfo {}

unsafe extern "C" fn cosmos_sigaction_handler(
    sig: c_int,
    info: *mut siginfo_t,
    _ucontext: *mut c_void,
) {
    let fault_addr = if !info.is_null() {
        unsafe { (*info).si_addr() as usize }
    } else {
        0
    };

    let sig_name = match sig {
        SIGSEGV => "SIGSEGV (Segmentation Fault)",
        SIGBUS => "SIGBUS (Bus Error)",
        SIGFPE => "SIGFPE (Floating Point Exception)",
        SIGILL => "SIGILL (Illegal Instruction)",
        _ => "UNKNOWN CRITICAL SIGNAL",
    };

    let in_zone = IN_GUARD_ZONE.with(|zone| *zone.borrow());

    if in_zone {
        eprintln!(
            "[COSMOS/Rust][CrashGuard] Trapped {} at 0x{:x}. Recovering via longjmp...",
            sig_name, fault_addr
        );

        JUMP_ENV.with(|cell| {
            if let Some(mut env) = *cell.borrow() {
                IN_GUARD_ZONE.with(|z| *z.borrow_mut() = false);
                unsafe { siglongjmp_c(&mut env, sig) };
            } else {
                unsafe { libc::abort() };
            }
        });
    } else {
        eprintln!(
            "[COSMOS/Rust][FATAL] {} at 0x{:x} occurred outside protected boundary.",
            sig_name, fault_addr
        );
        unsafe { libc::abort() };
    }
}

pub fn init_crash_guard() -> Result<()> {
    if GUARD_INSTALLED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    info!("[COSMOS/Rust] Mounting hardware signal crash guard...");

    unsafe {
        let mut sa: sigaction = MaybeUninit::zeroed().assume_init();
        sa.sa_sigaction = cosmos_sigaction_handler as *const () as usize;
        sa.sa_flags = SA_SIGINFO | SA_NODEFER | SA_ONSTACK;
        libc::sigemptyset(&mut sa.sa_mask);

        for &sig in &[SIGSEGV, SIGBUS, SIGFPE, SIGILL] {
            if sigaction(sig, &sa, std::ptr::null_mut()) != 0 {
                bail!("[COSMOS/Rust] Failed to install handler for signal: {}", sig);
            }
        }
    }

    info!("[COSMOS/Rust] Hardware crash guard successfully mounted.");
    Ok(())
}

pub fn run_guarded<F, R>(action: F) -> Result<R, CrashInfo>
where
    F: FnOnce() -> R,
{
    if !GUARD_INSTALLED.load(Ordering::Relaxed) {
        let _ = init_crash_guard();
    }

    let mut env = JmpBuf { _buffer: [0u8; 256] };

    let jump_ret = unsafe { sigsetjmp_c(&mut env, 1) };

    if jump_ret == 0 {
        JUMP_ENV.with(|cell| *cell.borrow_mut() = Some(env));
        IN_GUARD_ZONE.with(|zone| *zone.borrow_mut() = true);

        let result = action();

        IN_GUARD_ZONE.with(|zone| *zone.borrow_mut() = false);
        JUMP_ENV.with(|cell| *cell.borrow_mut() = None);

        Ok(result)
    } else {
        IN_GUARD_ZONE.with(|zone| *zone.borrow_mut() = false);
        JUMP_ENV.with(|cell| *cell.borrow_mut() = None);

        let sig_name = match jump_ret {
            SIGSEGV => "SIGSEGV",
            SIGBUS => "SIGBUS",
            SIGFPE => "SIGFPE",
            SIGILL => "SIGILL",
            _ => "UNKNOWN",
        };

        warn!("[COSMOS/Rust][CrashGuard] Trapped and recovered from signal {}", sig_name);

        Err(CrashInfo {
            signal: jump_ret,
            signal_name: sig_name,
            fault_addr: 0,
        })
    }
}