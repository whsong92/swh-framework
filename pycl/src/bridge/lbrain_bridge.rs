use anyhow::{Context, Result};
use jni::objects::{JByteBuffer, JString, JValue};
use jni::{InitArgsBuilder, JNIVersion, JavaVM};
use std::sync::Arc;
use tracing::info;

pub struct LbrainBridge {
    jvm: Option<Arc<JavaVM>>,
}

impl LbrainBridge {
    pub fn init() -> Result<Self> {
        // OS 환경변수 CLASSPATH 확인 및 .classpath 파일 보조
        let cp_from_file = std::fs::read_to_string("lbrain/.classpath")
            .or_else(|_| std::fs::read_to_string("../lbrain/.classpath"))
            .unwrap_or_default();

        let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let lbrain_src = current_dir.join("lbrain/src");
        let src_path = if lbrain_src.exists() {
            lbrain_src.to_string_lossy().to_string()
        } else {
            "lbrain/src".to_string()
        };

        let classpath = if let Ok(env_cp) = std::env::var("CLASSPATH") {
            format!("{}:{}", env_cp, src_path)
        } else if !cp_from_file.trim().is_empty() {
            format!("{}:{}", cp_from_file.trim(), src_path)
        } else {
            let home = std::env::var("HOME").unwrap_or_default();
            format!(
                "{}/.m2/repository/org/clojure/clojure/1.12.0/clojure-1.12.0.jar:{}/.m2/repository/org/clojure/data.json/2.5.0/data.json-2.5.0.jar:{}",
                home, home, src_path
            )
        };

        info!("[YAHWEH/Clojure] Launching JVM with classpath: {}", classpath);

        let cp_opt = format!("-Djava.class.path={}", classpath);
        let jvm_args = InitArgsBuilder::new()
            .version(JNIVersion::V8)
            .option(&cp_opt)
            .option("-Xss4m")
            .build()
            .context("[YAHWEH/Clojure] Failed to build JVM InitArgs")?;

        let jvm = JavaVM::new(jvm_args).context("[YAHWEH/Clojure] Failed to launch embedded JVM")?;
        info!("[YAHWEH/Clojure] Embedded JVM launched successfully.");

        let mut bridge = Self {
            jvm: Some(Arc::new(jvm)),
        };
        bridge.bootstrap_runtime()?;
        Ok(bridge)
    }

    fn bootstrap_runtime(&mut self) -> Result<()> {
        let jvm = self.jvm.as_ref().context("JVM not initialized")?;
        let mut env = jvm.attach_current_thread()?;

        // 스레드 ContextClassLoader를 SystemClassLoader로 설정하여 Clojure 동적 로더 연결 보장
        let system_class_loader = env.call_static_method(
            "java/lang/ClassLoader",
            "getSystemClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )?.l()?;

        let current_thread = env.call_static_method(
            "java/lang/Thread",
            "currentThread",
            "()Ljava/lang/Thread;",
            &[],
        )?.l()?;

        env.call_method(
            &current_thread,
            "setContextClassLoader",
            "(Ljava/lang/ClassLoader;)V",
            &[JValue::from(&system_class_loader)],
        )?;

        // lbrain.core & lbrain.memory 로드
        let core_ns = env.new_string("lbrain.core")?;
        let mem_ns = env.new_string("lbrain.memory")?;
        let clojure_core = env.new_string("clojure.core")?;
        let require_fn = env.new_string("require")?;

        let req_var = env.call_static_method(
            "clojure/lang/RT",
            "var",
            "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
            &[JValue::from(&clojure_core), JValue::from(&require_fn)],
        )?.l()?;

        let core_sym = env.call_static_method(
            "clojure/lang/RT",
            "readString",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::from(&core_ns)],
        )?.l()?;

        let mem_sym = env.call_static_method(
            "clojure/lang/RT",
            "readString",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::from(&mem_ns)],
        )?.l()?;

        env.call_method(
            &req_var,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::from(&core_sym)],
        )?;

        env.call_method(
            &req_var,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::from(&mem_sym)],
        )?;

        info!("[YAHWEH/Clojure] Runtime namespaces required (lbrain.core, lbrain.memory).");
        Ok(())
    }

    pub fn bind_shared_memory(&mut self, ptr: *mut f32, size: usize) -> Result<()> {
        let byte_len = size * std::mem::size_of::<f32>();
        let jvm = self.jvm.as_ref().context("JVM not initialized")?;
        let mut env = jvm.attach_current_thread()?;

        let direct_bb: JByteBuffer = unsafe { env.new_direct_byte_buffer(ptr as *mut u8, byte_len)? };

        let core_ns = env.new_string("lbrain.core")?;
        let bind_fn = env.new_string("bind-shared-arena")?;

        let var_obj = env.call_static_method(
            "clojure/lang/RT",
            "var",
            "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
            &[JValue::from(&core_ns), JValue::from(&bind_fn)],
        )?.l()?;

        env.call_method(
            &var_obj,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::from(&direct_bb)],
        )?;

        info!("[YAHWEH/Clojure] Zero-Copy DirectByteBuffer bound successfully: 0x{:x}", ptr as usize);
        Ok(())
    }

    pub fn dispatch(&self, opcode: &str, payload_json: &str) -> Result<String> {
        let jvm = self.jvm.as_ref().context("JVM not initialized")?;
        let mut env = jvm.attach_current_thread()?;

        let core_ns = env.new_string("lbrain.core")?;
        let dispatch_fn = env.new_string("dispatch")?;

        let var_obj = env.call_static_method(
            "clojure/lang/RT",
            "var",
            "(Ljava/lang/String;Ljava/lang/String;)Lclojure/lang/Var;",
            &[JValue::from(&core_ns), JValue::from(&dispatch_fn)],
        )?.l()?;

        let op_jstr = env.new_string(opcode)?;
        let payload_jstr = env.new_string(payload_json)?;

        let result_obj = env.call_method(
            &var_obj,
            "invoke",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::from(&op_jstr), JValue::from(&payload_jstr)],
        )?.l()?;

        let result_jstr: JString = result_obj.into();
        let res_str: String = env.get_string(&result_jstr)?.into();
        Ok(res_str)
    }

    pub fn ping(&self) -> Result<String> {
        self.dispatch("status", "{}")
    }
}