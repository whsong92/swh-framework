;; swh-framework/lbrain/src/lbrain/sandbox.clj
(ns lbrain.sandbox
  "swhAI YAHWEH 격리 기호 샌드박스 (Isolated Symbolic Sandbox).
   - Rust(pycl) 호스트 내부에서 호출되는 인-프로세스 라이브러리 모듈.
   - GraalVM Native Image 환경에서 바이트코드 동적 생성 없이 순수 AST 해석(SCI) 수행.
   - 외부 자연어/코드로부터 컴파일된 기호 규칙 및 상태 전이 식을 안전하게 격리 실행.
   - 위험한 시스템 호출(System, Thread, File IO)을 차단하고 6대 프리미티브 및 인과 전이 함수만 화이트리스트 노출."
  (:require [sci.core :as sci]
            [lbrain.primitives.type-hierarchy :as hierarchy]
            [lbrain.primitives.contradiction :as contra]
            [lbrain.causality.machine :as causality]))

;; -----------------------------------------------------------------------------
;; 1. SCI 화이트리스트 샌드박스 네임스페이스 바인딩
;; -----------------------------------------------------------------------------

(def ^:private sandbox-env
  {;; 기본 수학 및 산술 연산자
   'Math
   {'sin   #(Math/sin %)
    'cos   #(Math/cos %)
    'sqrt  #(Math/sqrt %)
    'abs   #(Math/abs (double %))
    'max   #(Math/max (double %1) (double %2))
    'min   #(Math/min (double %1) (double %2))}

   ;; YAHWEH 불변 공리계 및 Universal Primitives 안전 인터페이스
   'axiom
   {'subtype?            hierarchy/subtype?
    'has-attribute?      hierarchy/has-attribute?
    'verify-inclusion    hierarchy/verify-inclusion
    'has-contradiction?  contra/has-contradiction?
    'verify-opposition   contra/verify-opposition}

   ;; 인과 전이 머신 연계
   'causal
   {'transition-step      causality/transition-step
    'create-initial-state causality/create-initial-state}})

;; 불변의 격리 컨텍스트 생성 (위험한 클래스 로더, Thread, System 호출 원천 차단)
(defonce ^:dynamic *sandbox-ctx*
  (sci/init {:classes {'java.lang.Math java.lang.Math}
             :namespaces sandbox-env
             :allow-all false}))

;; -----------------------------------------------------------------------------
;; 2. 샌드박스 안전 실행 인터페이스 (라이브러리 API)
;; -----------------------------------------------------------------------------

(defn eval-sandboxed
  "바이트코드 생성 없이 SCI 샌드박스 내에서 S-식 문자열을 즉석 평가.
   - code-str: '(defn rule [x] ...)' 또는 단순 표현식
   - bindings: 평가 시 주입할 로컬 바인딩 맵 (예: {'x 10, 'current-state S})"
  ([code-str]
   (eval-sandboxed code-str {}))
  ([code-str bindings]
   (try
     (let [result (sci/binding [sci/out *out*
                                sci/err *err*]
                    (sci/eval-string code-str {:ctx *sandbox-ctx*
                                               :bindings bindings}))]
       {:status :success
        :result result})
     (catch Throwable e
       (println (format "[YAHWEH/Sandbox Error] Execution trapped: %s" (.getMessage e)))
       {:status :error
        :error (.getMessage e)}))))

(defn eval-predicate
  "샌드박스 내에서 참/거짓 판정 규칙을 실행하고 불리언 값으로 강제 정규화"
  [rule-code-str context-map]
  (let [eval-res (eval-sandboxed rule-code-str context-map)]
    (if (= :success (:status eval-res))
      (boolean (:result eval-res))
      (do
        (println (format "[YAHWEH/Sandbox] Predicate evaluation failed: %s" (:error eval-res)))
        false))))