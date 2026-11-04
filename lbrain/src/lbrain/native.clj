;; swh-framework/lbrain/src/lbrain/native.clj
(ns lbrain.native
  "swhAI YAHWEH GraalVM Native AOT (C-ABI) 진입점.
   - Release Mode: GraalVM native-image --shared 로 libyahweh.so 빌드 시 C 함수 심볼로 노출.
   - Rust(pycl)에서 extern \"C\" 로 직접 호출하여 JVM 오버헤드와 시그널 충돌을 완전히 제거.
   - sun.misc.Unsafe를 활용하여 Rust의 물리 포인터(*mut f32)를 무복사 0-오버헤드로 직접 조작."
  (:gen-class
   :name lbrain.native.YahwehNative
   :methods [^{:static true} [verify_memory_c [long long] int]
             ^{:static true} [bind_arena_c [long long] int]
             ^{:static true} [ping_c [] String]
             ^{:static true} [eval_sexp_c [String] String]])
  (:require [lbrain.memory :as mem]
            [lbrain.primitives.type-hierarchy :as hierarchy]
            [lbrain.primitives.contradiction :as contra])
  (:import [sun.misc Unsafe]))

;; -----------------------------------------------------------------------------
;; 1. Unsafe 인스턴스 획득 (Native Raw Memory Access)
;; -----------------------------------------------------------------------------

(def ^Unsafe unsafe
  (try
    (let [field (.getDeclaredField Unsafe "theUnsafe")]
      (.setAccessible field true)
      (.get field nil))
    (catch Throwable e
      (println "[YAHWEH/Native Error] Failed to obtain Unsafe instance: " (.getMessage e))
      nil)))

;; 네이티브 모드 공유 물리 아레나 기본 주소 및 크기
(defonce native-arena-ptr (atom 0))
(defonce native-arena-size (atom 0))

;; -----------------------------------------------------------------------------
;; 2. C-ABI 바인딩 및 메모리 무복사 직접 조작
;; -----------------------------------------------------------------------------

(defn -bind_arena_c
  "Rust(pycl/bridge/lbrain_bridge.rs)로부터 물리 포인터(mem-ptr)와 크기를 받아 네이티브 바인딩 수행"
  [^Long mem-ptr ^Long float-count]
  (try
    (if (or (zero? mem-ptr) (nil? unsafe))
      (do
        (println "[YAHWEH/Native Error] Invalid memory pointer or Unsafe unavailable.")
        (int -1))
      (do
        (reset! native-arena-ptr mem-ptr)
        (reset! native-arena-size float-count)
        (println (format "[YAHWEH/Native] Bound Raw Memory Pointer: 0x%X, Size: %d slots (%d MB) [Zero-Copy Native]"
                         mem-ptr float-count (quot (* float-count 4) (* 1024 1024))))
        (int 0)))
    (catch Throwable e
      (println "[YAHWEH/Native Error] bind_arena_c failed: " (.getMessage e))
      (int -2))))

(defn -verify_memory_c
  "Rust로부터 물리 포인터(mem-ptr)와 원소 개수를 받아 무복사 읽기/쓰기 핑퐁 검증 수행"
  [^Long mem-ptr ^Long float-count]
  (try
    (if (or (zero? mem-ptr) (nil? unsafe))
      (int -1)
      (let [;; 0번 슬롯(offset 0) 읽기
            curr-val (.getFloat unsafe (long mem-ptr))
            test-token (float 777.0)]
        (println (format "[YAHWEH/Native] Verifying Arena Memory at 0x%X. Read val: %.2f" mem-ptr curr-val))
        ;; Rust 및 Python이 즉시 감지할 수 있도록 777.0 기록
        (.putFloat unsafe (long mem-ptr) test-token)
        (println (format "[YAHWEH/Native] Verification Token %.2f written to 0x%X." test-token mem-ptr))
        (int 0)))
    (catch Throwable e
      (println "[YAHWEH/Native Error] verify_memory_c exception: " (.getMessage e))
      (int -1))))

;; -----------------------------------------------------------------------------
;; 3. 네이티브 헬스체크 및 S-식 평가 C-ABI
;; -----------------------------------------------------------------------------

(defn -ping_c
  "Release AOT 모드에서 Rust ping 호출에 응답"
  []
  "YAHWEH Native AOT online (GraalVM native-image C-ABI)")

(defn -eval_sexp_c
  "Rust에서 extern \"C\"를 통해 전달한 S-expression 문자열을 평가하고 결과를 문자열로 반환.
   타입 계층(⊂) 및 모순(¬) 기호 공리계 함수와 직결."
  [^String sexp-str]
  (try
    (if (nil? sexp-str)
      "(:error \"Null S-expression\")"
      (let [trimmed (.trim sexp-str)]
        ;; 간단한 S-식 패턴 매칭 또는 온톨로지 검증 직결
        (cond
          (.startsWith trimmed "(verify-inclusion")
          (str (eval (read-string (str "(lbrain.primitives.type-hierarchy/" (.substring trimmed 1))))))

          (.startsWith trimmed "(verify-opposition")
          (str (eval (read-string (str "(lbrain.primitives.contradiction/" (.substring trimmed 1))))))

          :else
          (let [res (eval (read-string trimmed))]
            (pr-str res)))))
    (catch Throwable e
      (format "(:error \"%s\")" (.getMessage e)))))