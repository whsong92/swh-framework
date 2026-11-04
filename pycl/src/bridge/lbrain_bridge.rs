use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::info;

#[cfg(feature = "dev-jvm")]
use jni::objects::{GlobalRef, JByteBuffer, JObject, JString, JValue};
#[cfg(feature = "dev-jvm")]
use jni::{InitArgsBuilder, JNIVersion, JavaVM};

#[cfg(not(feature = "dev-jvm"))]
use std::ffi::{CStr, CString};
#[cfg(not(feature = "dev-jvm"))]
use std::os::raw::c_char;

/// [YAHWEH/Clojure] 기호 추론 및 공리계 엔진 브리지
/// - Dev Mode: GraalVM JVM 위에서 JNI를 통해 REPL 핫리로드 지원
/// - Release Mode: GraalVM native-image 빌드물(libyahweh.so) C-ABI 직결
pub struct LbrainBridge {
    #[cfg(feature = "dev-jvm")]
    jvm: JavaVM,

    #[cfg(feature = "dev-jvm")]
    /// JVM 전체에서 유지되는 Clojure DynamicClassLoader 싱글톤 참조
    shared_dcl: GlobalRef,

    #[cfg(not(feature = "dev-jvm"))]
    _native_handle: (),
}

impl LbrainBridge {
    /// 이종 파이프라인 부트스트랩
    pub fn init() -> Result<Self> {
        #[cfg(feature = "dev-jvm")]
        {
            info!("[COSMOS/Rust] Initializing YAHWEH via GraalVM JVM (Dev Mode)...");

            let candidate_dirs = [
                Path::new("lbrain").to_path_buf(),
                Path::new("../lbrain").to_path_buf(),
                Path::new("../../lbrain").to_path_buf(),
                Path::new("/workspace/swh-framework/lbrain").to_path_buf(),
            ];

            let lbrain_dir = candidate_dirs
                .iter()
                .find(|p| p.exists() && p.join("deps.edn").exists())
                .cloned()
                .unwrap_or_else(|| Path::new("lbrain").to_path_buf());

            let lbrain_canon = lbrain_dir.canonicalize().unwrap_or(lbrain_dir);

            // 1. clj -Spath 로 동적 클래스패스 추출
            let output = Command::new("clj")
                .arg("-Spath")
                .current_dir(&lbrain_canon)
                .output()
                .context("[YAHWEH/Clojure] Failed to invoke clj tool for classpath resolution")?;

            if !output.status.success() {
                anyhow::bail!(
                    "[YAHWEH/Clojure] clj classpath resolution error: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let resolved_cp = String::from_utf8(output.stdout)?.trim().to_string();
            let src_path = lbrain_canon.join("src").to_string_lossy().to_string();
            let full_cp = format!("-Djava.class.path={}:{}", resolved_cp, src_path);

            info!("[YAHWEH/Clojure] Classpath resolved successfully.");

            // 2. JVM 기동 인자 조립 (-Xrs 로 하드웨어 시그널 크래시 가드 충돌 방지)
            let jvm_args = InitArgsBuilder::new()
                .version(JNIVersion::V8)
                .option(&full_cp)
                .option("-Xrs")
                .build()
                .context("[COSMOS/Rust] Failed to assemble JVM InitArgs")?;

            let jvm = JavaVM::new(jvm_args)
                .context("[COSMOS/Rust] Failed to spawn GraalVM JVM instance")?;

            // 3. 스레드 ContextClassLoader 정상 결속 및 네임스페이스 로드
            let shared_dcl = {
                let mut env = jvm.attach_current_thread()?;

                // (1) SystemClassLoader 획득
                let sys_loader = env
                    .call_static_method(
                        "java/lang/ClassLoader",
                        "getSystemClassLoader",
                        "()Ljava/lang/ClassLoader;",
                        &[],
                    )?
                    .l()?;

                // (2) DynamicClassLoader 생성 및 GlobalRef 승격
                let dcl_class = env.find_class("clojure/lang/DynamicClassLoader")?;
                let dcl_instance = env.new_object(
                    &dcl_class,
                    "(Ljava/lang/ClassLoader;)V",
                    &[JValue::from(&sys_loader)],
                )?;
                let global_dcl = env.new_global_ref(dcl_instance)?;

                // (3) 현재 OS 스레드의 ContextClassLoader에 DynamicClassLoader 바인딩
                let current_thread = env
                    .call_static_method(
                        "java/lang/Thread",
                        "currentThread",
                        "()Ljava/lang/Thread;",
                        &[],
                    )?
                    .l()?;

                env.call_method(
                    &current_thread,
                    "setContextClassLoader",
                    "(Ljava/lang/ClassLoader;)V",
                    &[JValue::from(&global_dcl)],
                )?;

                // (4) clojure.core/require 로드 (Compiler.LOADER 변조 없이 정상 호출)
                let core_ns = env.new_string("clojure.core")?;
                let require_fn = env.new_string("require")?;
                let require_var = env
                    .call_static_method(
                        "clojure/lang/RT",
                        "var",
                        "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
                        &[JValue::from(&core_ns), JValue::from(&require_fn)],
                    )?
                    .l()?;

                for ns in &["lbrain.core", "lbrain.dynamic-loader"] {
                    let ns_obj = env.new_string(*ns)?;
                    let sym = env
                        .call_static_method(
                            "clojure/lang/Symbol",
                            "intern",
                            "(Ljava/lang/String;)Lclojure/lang/Symbol;",
                            &[JValue::from(&ns_obj)],
                        )?
                        .l()?;

                    env.call_method(
                        &require_var,
                        "invoke",
                        "(Ljava/lang/Object;)Ljava/lang/Object;",
                        &[JValue::from(&sym)],
                    )?;
                }

                global_dcl
            };

            info!("[COSMOS/Rust] YAHWEH Dev-JVM initialized with Singleton DynamicClassLoader.");
            Ok(Self { jvm, shared_dcl })
        }

        #[cfg(not(feature = "dev-jvm"))]
        {
            info!("[COSMOS/Rust] Initializing YAHWEH via libyahweh.so C-ABI (Release Mode)...");
            extern "C" {
                fn yahweh_native_init() -> i32;
            }
            let status = unsafe { yahweh_native_init() };
            if status != 0 {
                anyhow::bail!("[YAHWEH/Clojure] Native AOT C-ABI init failed: code {}", status);
            }
            Ok(Self { _native_handle: () })
        }
    }

    #[cfg(feature = "dev-jvm")]
    /// 호출 스레드에 싱글톤 DynamicClassLoader 주입
    fn ensure_thread_context(&self, env: &mut jni::JNIEnv) -> Result<()> {
        let current_thread = env
            .call_static_method(
                "java/lang/Thread",
                "currentThread",
                "()Ljava/lang/Thread;",
                &[],
            )?
            .l()?;

        env.call_method(
            &current_thread,
            "setContextClassLoader",
            "(Ljava/lang/ClassLoader;)V",
            &[JValue::from(&self.shared_dcl)],
        )?;

        Ok(())
    }

    /// L0 Shared Arena 물리 메모리를 Clojure 환경에 무복사 바인딩 (DirectByteBuffer)
    pub fn bind_shared_memory(&mut self, ptr: *mut f32, size: usize) -> Result<()> {
        let byte_len = size * std::mem::size_of::<f32>();

        #[cfg(feature = "dev-jvm")]
        {
            let mut env = self.jvm.attach_current_thread()?;
            self.ensure_thread_context(&mut env)?;

            let direct_bb: JByteBuffer =
                unsafe { env.new_direct_byte_buffer(ptr as *mut u8, byte_len)? };

            let core_ns = env.new_string("lbrain.core")?;
            let bind_fn = env.new_string("bind-shared-arena")?;

            let var_obj = env
                .call_static_method(
                    "clojure/lang/RT",
                    "var",
                    "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
                    &[JValue::from(&core_ns), JValue::from(&bind_fn)],
                )?
                .l()?;

            let _ = env.call_method(
                &var_obj,
                "invoke",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[JValue::from(&direct_bb)],
            )?;

            info!(
                "[YAHWEH/Clojure] Zero-Copy DirectByteBuffer bound: 0x{:x} ({} MB)",
                ptr as usize,
                byte_len / (1024 * 1024)
            );
            Ok(())
        }

        #[cfg(not(feature = "dev-jvm"))]
        {
            extern "C" {
                fn yahweh_bind_memory(ptr: *mut f32, bytes: usize) -> i32;
            }
            let code = unsafe { yahweh_bind_memory(ptr, byte_len) };
            if code != 0 {
                anyhow::bail!("[YAHWEH/Clojure] Failed to bind native memory: code {}", code);
            }
            Ok(())
        }
    }

    /// 헬스체크 핑
    pub fn ping(&self) -> Result<String> {
        self.eval_sexp("(lbrain.core/yahweh \"PING\")")
    }

    /// 임의의 S-expression 평가
    pub fn eval_sexp(&self, sexp: &str) -> Result<String> {
        #[cfg(feature = "dev-jvm")]
        {
            let mut env = self.jvm.attach_current_thread()?;
            self.ensure_thread_context(&mut env)?;

            let core_ns = env.new_string("clojure.core")?;
            let eval_fn = env.new_string("eval")?;
            let read_fn = env.new_string("read-string")?;

            let eval_var = env
                .call_static_method(
                    "clojure/lang/RT",
                    "var",
                    "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
                    &[JValue::from(&core_ns), JValue::from(&eval_fn)],
                )?
                .l()?;

            let read_var = env
                .call_static_method(
                    "clojure/lang/RT",
                    "var",
                    "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
                    &[JValue::from(&core_ns), JValue::from(&read_fn)],
                )?
                .l()?;

            let sexp_jstr = env.new_string(sexp)?;
            let parsed_form = env.call_method(
                &read_var,
                "invoke",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[JValue::from(&sexp_jstr)],
            )?;

            let evaluated = env.call_method(
                &eval_var,
                "invoke",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[JValue::from(&parsed_form.l()?)],
            )?;

            let ret_obj: JObject = evaluated.l()?;
            if ret_obj.is_null() {
                return Ok("nil".to_string());
            }

            let to_string_val = env
                .call_method(&ret_obj, "toString", "()Ljava/lang/String;", &[])?
                .l()?;

            let jstr: JString = to_string_val.into();
            let result_str: String = env.get_string(&jstr)?.into();
            Ok(result_str)
        }

        #[cfg(not(feature = "dev-jvm"))]
        {
            extern "C" {
                fn yahweh_eval(s: *const c_char) -> *const c_char;
            }
            let c_str = CString::new(sexp)?;
            let ptr = unsafe { yahweh_eval(c_str.as_ptr()) };
            if ptr.is_null() {
                return Ok("nil".to_string());
            }
            let out = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
            Ok(out)
        }
    }

    /// 포함 (⊂): 타입 계층 유효성 검증
    pub fn verify_inclusion(&self, sub_concept: &str, super_concept: &str) -> Result<bool> {
        let sexp = format!(
            "(try (lbrain.core/verify-inclusion \"{}\" \"{}\") (catch Exception _ false))",
            sub_concept, super_concept
        );
        let res = self.eval_sexp(&sexp)?;
        Ok(res.trim() == "true")
    }

    /// 반대/부정 (¬): 상호 배타 공리계 모순 검증
    pub fn verify_opposition(&self, traits: &[&str]) -> Result<bool> {
        let traits_vec = traits
            .iter()
            .map(|t| format!("\"{}\"", t))
            .collect::<Vec<_>>()
            .join(" ");

        let sexp = format!(
            "(try (lbrain.core/verify-opposition [{}]) (catch Exception _ false))",
            traits_vec
        );
        let res = self.eval_sexp(&sexp)?;
        Ok(res.trim() == "true")
    }
}