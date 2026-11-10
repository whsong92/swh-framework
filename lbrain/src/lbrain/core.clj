;; lbrain/src/lbrain/core.clj
(ns lbrain.core
  "swhAI YAHWEH Core Runtime Infrastructure.
   - L0 Shared Memory DirectByteBuffer 제어
   - 동적 S-식 런타임 컴파일 및 마운트 (SCI)
   - Rust 스토리지 및 BUDDHA 인스턴스 연동"
  (:require [lbrain.memory :as mem]
            [lbrain.dynamic-loader :as dyn]
            [clojure.data.json :as json]))

;; =============================================================================
;; 1. 인스턴스 라이프사이클 및 상태 디스크립터
;; =============================================================================

(defn bind-shared-arena
  "Rust 호스트(pycl)에서 전달받은 DirectByteBuffer 제로카피 바인딩"
  [^java.nio.ByteBuffer byte-buffer]
  (mem/bind-raw-memory byte-buffer))

(defn get-engine-status
  "YAHWEH 기호 추론 인프라 상태 반환"
  []
  (let [bound? (mem/is-bound?)
        backend (if (System/getProperty "java.vm.name") "graalvm-dev" "native-c-abi")
        modules (mapv str (dyn/list-mounted-skills))]
    {:engine "YAHWEH"
     :backend backend
     :arena {:bound bound?
             :slots (mem/get-capacity-slots)
             :capacity_bytes (mem/get-capacity-bytes)}
     :mounted_modules_count (count modules)
     :mounted_modules modules}))

;; =============================================================================
;; 2. 통합 인프라 디스패처 (Dispatch)
;; =============================================================================

(defn dispatch
  "Rust 호스트(pycl)에서 단일 진입점으로 호출되는 JSON 프로토콜 분기기"
  [^String opcode ^String payload-json]
  (let [payload (try
                  (json/read-str payload-json :key-fn keyword)
                  (catch Exception _ {}))]
    (json/write-str
      (case opcode
        ;; 1. 런타임 상태 질의
        "status"
        (get-engine-status)

        ;; 2. 동적 코드 마운트 및 실행 (Infrastructure)
        "mount_module"
        (let [mod-name (:module_name payload)
              code-str (:code payload)
              success (dyn/register-symbolic-skill mod-name code-str)]
          {:status (if success "SUCCESS" "FAILED") :module mod-name})

        "call_module"
        (let [mod-name (:module_name payload)
              args (cond
                     (contains? payload :args) (:args payload)
                     (contains? payload :arg)  [(:arg payload)]
                     :else [])]
          {:module mod-name :result (apply dyn/invoke-symbolic-skill mod-name args)})

        "unmount_module"
        (let [mod-name (:module_name payload)]
          {:status "SUCCESS" :unmounted (dyn/unregister-symbolic-skill mod-name)})

        ;; 3. L0 아레나 슬라이스 메모리 I/O
        "read_tensor"
        (if-not (mem/is-bound?)
          {:error "Shared arena is not bound"}
          (let [offset (:offset payload)
                length (:length payload)
                data (mem/read-slice offset length)]
            {:offset offset :length length :data data}))

        "write_tensor"
        (if-not (mem/is-bound?)
          {:error "Shared arena is not bound"}
          (let [offset (:offset payload)
                data (:data payload)
                written (mem/write-slice! offset data)]
            {:offset offset :written_slots written}))

        ;; 예외 처리
        {:error (str "[YAHWEH Infrastructure Error] Unsupported opcode: " opcode)}))))