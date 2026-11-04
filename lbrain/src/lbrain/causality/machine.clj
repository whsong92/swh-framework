(ns lbrain.causality.machine
  (:require [lbrain.primitives.contradiction :as contra]
            [lbrain.primitives.type-hierarchy :as hier]
            [clojure.string :as str]))

;; =============================================================================
;; 1. 인과 상태(State) 및 액션(Action) 데이터 모델
;; =============================================================================

;; swhAI 상태 머신 내부 인과 노드 레코드
;; - step       : 전이 단계 (t)
;; - concepts   : 현재 활성화된 개념 심볼 세트
;; - traits     : 현재 누적된 속성 및 상태 공리 세트
;; - context    : 수치적/메타데이터 맵
;; - history    : 과거 전이 궤적 (역추적 및 반사실적 시뮬레이션용)
;; - status     : :ok | :pruned | :halted
;; - prune-meta : 절단된 이유 및 위반된 모순 공리 정보
(defrecord CognitiveState [step concepts traits context history status prune-meta])

(defn create-initial-state
  "초기 인과 머신 상태 생성"
  ([] (create-initial-state [] [] {}))
  ([init-concepts init-traits context]
   (->CognitiveState
     0
     (set (mapv #(keyword (str/replace (str %) #"^:" "")) init-concepts))
     (set (mapv #(keyword (str/replace (str %) #"^:" "")) init-traits))
     (or context {})
     []
     :ok
     nil)))

;; =============================================================================
;; 2. 단일 단계 인과 전이 함수 (Transition Step)
;; =============================================================================

(defn- validate-action-spec
  "액션 맵의 최소 필수 키 검증"
  [action]
  (and (map? action)
       (or (contains? action :add-traits)
           (contains? action :remove-traits)
           (contains? action :add-concepts)
           (contains? action :remove-concepts)
           (contains? action :op))))

(defn step
  "단일 단계 인과 전이 함수: S_{t+1} = delta(S_t, Action)
   새로운 속성이 추가될 때 YAHWEH 공리계(contra/has-contradiction?)를 즉시 평가하여
   모순 감지 시 실행 엣지를 런타임에 즉시 절단(Pruning)하고 상태를 보존함."
  [^CognitiveState state action]
  (if (not= (:status state) :ok)
    ;; 이미 절단되었거나 중단된 상태는 전이하지 않음
    state
    (if-not (validate-action-spec action)
      (assoc state :status :halted :prune-meta {:reason :invalid-action-spec :action action})
      (let [curr-traits   (:traits state)
            curr-concepts (:concepts state)
            raw-add-traits (mapv #(keyword (str/replace (str %) #"^:" "")) (:add-traits action))
            raw-rm-traits  (set (mapv #(keyword (str/replace (str %) #"^:" "")) (:remove-traits action)))
            raw-add-concs  (mapv #(keyword (str/replace (str %) #"^:" "")) (:add-concepts action))
            raw-rm-concs   (set (mapv #(keyword (str/replace (str %) #"^:" "")) (:remove-concepts action)))

            ;; 1. 신규 특성 후보 집합 산출
            candidate-traits (-> curr-traits
                                 (into raw-add-traits)
                                 (as-> ts (remove raw-rm-traits ts))
                                 set)

            ;; 2. YAHWEH 율법: 상호 배타 모순 검증 (¬ Pruning)
            conflict-detected? (contra/has-contradiction? (vec candidate-traits))]

        (if conflict-detected?
          ;; [공리적 모순 감지] 실행 엣지 즉각 절단 (Pruning)
          (do
            (println (format "[YAHWEH/Clojure] Axiomatic contradiction triggered by action '%s'. Edge PRUNED."
                             (or (:op action) "anonymous-action")))
            (assoc state
                   :step (inc (:step state))
                   :status :pruned
                   :prune-meta {:reason :contradiction-detected
                                :conflicting-traits (vec candidate-traits)
                                :action action}
                   :history (conj (:history state)
                                  {:step (:step state)
                                   :action action
                                   :verdict :pruned})))

          ;; [공리 통과] 다음 상태로 정상 전이
          (let [candidate-concepts (-> curr-concepts
                                       (into raw-add-concs)
                                       (as-> cs (remove raw-rm-concs cs))
                                       set)
                next-context (merge (:context state) (:update-context action))]
            (assoc state
                   :step (inc (:step state))
                   :concepts candidate-concepts
                   :traits candidate-traits
                   :context next-context
                   :status :ok
                   :prune-meta nil
                   :history (conj (:history state)
                                  {:step (:step state)
                                   :action action
                                   :verdict :accepted}))))))))

;; =============================================================================
;; 3. recur 기반 재귀 인과 시뮬레이터 (Counterfactual Causality Engine)
;; =============================================================================

(defn simulate-causality
  "주어진 초기 상태로부터 일련의 액션 시퀀스를 순차 적용하는 재귀 인과 루프.
   swhAI의 핵심: recur 기반 스택 소비 없는 불변 전이 추적."
  [initial-state action-sequence]
  (loop [curr-state initial-state
         actions    (seq action-sequence)]
    (if (or (empty? actions)
            (not= (:status curr-state) :ok))
      ;; 시퀀스가 종료되었거나 중간에 모순으로 절단(:pruned)된 경우 최종 상태 반환
      curr-state
      (let [next-act (first actions)
            next-st  (step curr-state next-act)]
        (recur next-st (rest actions))))))

(defn counterfactual-simulate
  "반사실적 추론 (What-if):
   기준 상태(base-state)에서 분기 액션(hypothetical-action)을 취했을 때
   모순 없이 유효한 미래가 성립 가능한지 시뮬레이션하고 성공 여부를 반환."
  [base-state hypothetical-actions]
  (let [isolated-state (assoc base-state :history [])
        sim-result     (simulate-causality isolated-state hypothetical-actions)]
    {:feasible? (= (:status sim-result) :ok)
     :final-step (:step sim-result)
     :pruned?    (= (:status sim-result) :pruned)
     :reason     (get-in sim-result [:prune-meta :reason])
     :state      sim-result}))

;; =============================================================================
;; 4. C-ABI / Rust Bridge 연동 진입점 (LbrainBridge Interface)
;; =============================================================================

(defn eval-causal-pipeline
  "Rust pycl 가상 머신에서 JSON 형태의 상태/액션을 전달받아 파이프라인을 실행하는 진입점"
  [init-traits-vec action-maps-vec]
  (try
    (let [init-st (create-initial-state [] init-traits-vec {})
          res     (simulate-causality init-st action-maps-vec)]
      {:success true
       :status (name (:status res))
       :step (:step res)
       :final-traits (mapv name (:traits res))
       :prune-meta (:prune-meta res)})
    (catch Exception e
      (println "[YAHWEH/Clojure][ERROR] Causal simulation failed:" (.getMessage e))
      {:success false
       :status "error"
       :message (.getMessage e)})))