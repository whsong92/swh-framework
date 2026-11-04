(ns lbrain.dynamic-loader
  (:require [lbrain.primitives.type-hierarchy :as hierarchy]
            [lbrain.primitives.contradiction :as contradiction]
            [clojure.string :as str]))

;; =============================================================================
;; 1. 동적 스킬 레지스트리 (In-Memory Hot-Patching Registry)
;; =============================================================================

(defonce ^:dynamic *skill-registry* (atom {}))

;; 기본 탑재 코어 기호 프리미티브 매핑 테이블 (17번째 줄 심볼 매핑)
(def default-primitive-registry
  {:primitive/inclusion   (fn [sub sup] (hierarchy/subtype? sub sup))
   :primitive/opposition  (fn [traits] (contradiction/verify-opposition traits))
   :primitive/addition    (fn [a b] (str "(+ " a " " b ")"))
   :primitive/subtraction (fn [a b] (str "(- " a " " b ")"))})

;; =============================================================================
;; 2. S-식 동적 컴파일 및 마운트 (Hot-Reloading)
;; =============================================================================

(defn register-symbolic-skill
  "문자열 형태의 S-expression 클로저를 런타임에 즉시 평가하여 레지스트리에 마운트"
  [skill-name-str sexp-code-str]
  (try
    (let [skill-key (keyword (str/replace (str skill-name-str) #"^:" ""))
          parsed-fn (binding [*ns* (find-ns 'lbrain.dynamic-loader)]
                      (eval (read-string sexp-code-str)))]
      (swap! *skill-registry* assoc skill-key parsed-fn)
      (println (format "[YAHWEH/Clojure] Symbolic skill '%s' compiled and mounted." skill-key))
      true)
    (catch Exception e
      (println (format "[YAHWEH/Clojure][ERROR] Skill registration failed for '%s': %s"
                       skill-name-str (.getMessage e)))
      false)))

(defn invoke-symbolic-skill
  "등록된 동적 스킬 실행"
  [skill-name-str & args]
  (let [skill-key (keyword (str/replace (str skill-name-str) #"^:" ""))]
    (if-let [f (or (get @*skill-registry* skill-key)
                   (get default-primitive-registry skill-key))]
      (apply f args)
      (throw (ex-info (str "Skill not found: " skill-key) {:skill skill-key})))))

(defn list-mounted-skills
  "현재 마운트된 모든 기호 스킬 키 목록 반환"
  []
  (keys @*skill-registry*))