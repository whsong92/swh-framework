;; lbrain/src/lbrain/causality/machine.clj
(ns lbrain.causality.machine
  (:require [lbrain.dynamic-loader :as dyn]
            [clojure.string :as str]))

;; =============================================================================
;; 1. 인과 상태(State) 및 액션(Action) 데이터 모델
;; =============================================================================

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

(defn- check-conflict
  "액션 내 자체 검증 술어 또는 동적 스킬(*skill-registry*) 기반 모순 검사"
  [traits action]
  (cond
    ;; 1. 액션에 자체 검증 함수가 포함된 경우
    (fn? (:validator action))
    (not ((:validator action) traits))

    ;; 2. 동적 스킬로 등록된 모순 검증기가 있는 경우
    (contains? (set (dyn/list-mounted-skills)) :rule/contradiction)
    (try
      (not (dyn/invoke-symbolic-skill :rule/contradiction traits))
      (catch Exception _ false))

    ;; 3. 기본값: 고정 룰 없이 통과
    :else false))

(defn step
  "단일 단계 인과 전이 함수: S_{t+1} = delta(S_t, Action)"
  [^CognitiveState state action]
  (if (not= (:status state) :ok)
    state
    (if-not (validate-action-spec action)
      (assoc state :status :halted :prune-meta {:reason :invalid-action-spec :action action})
      (let [curr-traits    (:traits state)
            curr-concepts  (:concepts state)
            raw-add-traits (mapv #(keyword (str/replace (str %) #"^:" "")) (:add-traits action))
            raw-rm-traits  (set (mapv #(keyword (str/replace (str %) #"^:" "")) (:remove-traits action)))
            raw-add-concs  (mapv #(keyword (str/replace (str %) #"^:" "")) (:add-concepts action))
            raw-rm-concs   (set (mapv #(keyword (str/replace (str %) #"^:" "")) (:remove-concepts action)))

            ;; 신규 특성 후보 집합
            candidate-traits (-> curr-traits
                                 (into raw-add-traits)
                                 (as-> ts (remove raw-rm-traits ts))
                                 set)

            ;; 모순 검증 (동적 검증기 기반)
            conflict-detected? (check-conflict candidate-traits action)]

        (if conflict-detected?
          (do
            (println (format "[YAHWEH/Clojure] Dynamic contradiction triggered by action '%s'. Edge PRUNED."
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
;; 3. recur 기반 재귀 인과 시뮬레이터
;; =============================================================================

(defn simulate-causality
  "주어진 초기 상태로부터 일련의 액션 시퀀스를 순차 적용하는 재귀 인과 루프"
  [initial-state action-sequence]
  (loop [curr-state initial-state
         actions    (seq action-sequence)]
    (if (or (empty? actions)
            (not= (:status curr-state) :ok))
      curr-state
      (let [next-act (first actions)
            next-st  (step curr-state next-act)]
        (recur next-st (rest actions))))))

(defn counterfactual-simulate
  "반사실적 추론 (What-if): 가상 분기 성립 여부 검증"
  [base-state hypothetical-actions]
  (let [isolated-state (assoc base-state :history [])
        sim-result     (simulate-causality isolated-state hypothetical-actions)]
    {:feasible? (= (:status sim-result) :ok)
     :final-step (:step sim-result)
     :pruned?    (= (:status sim-result) :pruned)
     :reason     (get-in sim-result [:prune-meta :reason])
     :state      sim-result}))

;; =============================================================================
;; 4. C-ABI / Rust Bridge 연동 진입점
;; =============================================================================

(defn eval-causal-pipeline
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