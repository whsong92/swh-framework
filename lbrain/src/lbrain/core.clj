;; lbrain/src/lbrain/core.clj
(ns lbrain.core
  (:require [lbrain.memory :as mem]
            [lbrain.causality.machine :as causality]
            [lbrain.dynamic-loader :as dyn]
            [clojure.data.json :as json]))

;; =============================================================================
;; 1. L0 메모리 바인딩 및 런타임 상태 디스크립터
;; =============================================================================

(defn bind-shared-arena
  "Rust 호스트 브리지(lbrain_bridge.rs) 호출 호환용 바인딩 래퍼"
  [^java.nio.ByteBuffer byte-buffer]
  (mem/bind-raw-memory byte-buffer))

(defn get-engine-status
  "YAHWEH 기호 추론 엔진 상태 반환 (Python의 BuddhaCore.get_status와 대칭)"
  []
  (let [bound? (mem/is-bound?)
        backend (if (System/getProperty "java.vm.name") "graalvm-dev" "native-c-abi")
        skills (mapv str (dyn/list-mounted-skills))]
    {:engine "YAHWEH"
     :backend backend
     :arena {:bound bound?
             :slots (mem/get-capacity-slots)
             :capacity_bytes (mem/get-capacity-bytes)}
     :mounted_skills_count (count skills)
     :mounted_skills skills}))

;; =============================================================================
;; 2. 통합 디스패처 (Dispatch)
;; =============================================================================

(defn dispatch
  "Rust 호스트(pycl)에서 호출하는 단일 진입 분기점 (JSON 입출력 규격).
   하드코딩된 대수/기하학 연산자를 배제하고 상태 질의, 인과 추론, 동적 스킬, 텐서 I/O 처리."
  [^String opcode ^String payload-json]
  (let [payload (try
                  (json/read-str payload-json :key-fn keyword)
                  (catch Exception _ {}))]
    (json/write-str
     (case opcode
       ;; 1. 런타임 상태 질의
       "status"
       (get-engine-status)

       ;; 2. 동적 스킬 마운트 및 실행 (dynamic_loader 실제 심볼에 매핑)
       "mount_skill"
       (let [s-name (:skill_name payload)
             code-str (:code payload)
             success (dyn/register-symbolic-skill s-name code-str)]
         {:status (if success "SUCCESS" "FAILED") :skill s-name})

       "dynamic_call"
       (let [s-name (:skill_name payload)
             arg (:arg payload)]
         {:skill s-name :result (dyn/invoke-symbolic-skill s-name arg)})

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

       ;; 4. 기호 인과 추론 머신 (Causality State Machine)
       "causal_step"
       (causality/step (:state payload) (:action payload))

       "causal_pipeline"
       (causality/eval-causal-pipeline (:init payload) (:actions payload))

       ;; 예외 처리
       {:error (str "[YAHWEH Core Error] Unsupported opcode: " opcode)}))))