(ns lbrain.core
  (:require [lbrain.memory :as mem]
            [lbrain.primitives.type-hierarchy :as hier]
            [lbrain.primitives.contradiction :as contra]
            [lbrain.causality.machine :as causality]
            [clojure.string :as str]))

;; =============================================================================
;; 1. L0 Zero-Copy 메모리 바인딩 및 상태
;; =============================================================================

(defonce ^:dynamic *shared-arena* (atom nil))

(defn bind-shared-arena
  "Rust 호스트(pycl)에서 전달한 DirectByteBuffer를 Clojure 런타임에 제로카피로 바인딩"
  [^java.nio.ByteBuffer byte-buffer]
  (reset! *shared-arena* byte-buffer)
  (println (format "[YAHWEH/Clojure] L0 DirectByteBuffer bound successfully. Capacity: %d bytes (Direct: %s)"
                   (.capacity byte-buffer)
                   (.isDirect byte-buffer)))
  true)

(defn get-shared-slot
  "L0 아레나의 특정 f32 슬롯 값을 무복사로 직접 읽기"
  [slot-idx]
  (if-let [^java.nio.ByteBuffer bb @*shared-arena*]
    (let [byte-offset (* (int slot-idx) 4)]
      (if (< (+ byte-offset 4) (.capacity bb))
        (str (.getFloat bb byte-offset))
        "OUT_OF_BOUNDS"))
    "UNBOUND"))

;; =============================================================================
;; 2. YAHWEH 코어 헬스체크 및 기호 엔진 진입점
;; =============================================================================

(defn yahweh
  "Rust 호스트 및 REPL 핑퐁 진입점"
  [signal]
  (let [sig (str signal)]
    (cond
      (= sig "PING")
      (str "PONG: YAHWEH Symbolic Engine Active | Arena: "
           (if @*shared-arena* "BOUND" "UNBOUND"))

      (= sig "STATUS")
      (str "{:status :ok :engine :yahweh :backend :graalvm-dev}")

      :else
      (str "ACK: " sig))))

;; =============================================================================
;; 3. Universal Primitives (6대 기본 대수 연산 모델) 기호 검증 진입점
;; =============================================================================

(defn verify-inclusion
  "포함 (⊂, Inclusion): 타입 계층 및 온톨로지 유효성 검증"
  [sub-concept super-concept]
  (hier/verify-inclusion sub-concept super-concept))

(defn verify-opposition
  "반대/부정 (¬, Negation): 상호 배타 공리계 모순 검증 (모순 감지 시 false)"
  [traits]
  (contra/verify-opposition traits))

;; =============================================================================
;; 4. 재귀적 인과 모델 (Recursive Causality Model) 전이 실행부
;; =============================================================================

(defn transition-step
  "단일 인과 상태 전이: S_{t+1} = causality/step(S_t, Action)
   114번째 줄 심볼 불일치 해소: causality/step 직접 호출"
  [current-state action-map]
  (causality/step current-state action-map))

(defn eval-causal-pipeline
  "Rust pycl 가상 머신에서 JSON 파라미터로 호출하는 복합 인과 전이 진입점"
  [init-traits action-maps]
  (causality/eval-causal-pipeline init-traits action-maps))

;; =============================================================================
;; 5. 네임스페이스 부트스트랩 완료 신호
;; =============================================================================

(println "[YAHWEH/Clojure] Symbolic Engine (YAHWEH) core initialized with 6 Primitives & Causality Engine.")