;; lbrain/src/lbrain/dynamic_loader.clj
(ns lbrain.dynamic-loader
  "swhAI YAHWEH 런타임 동적 기호 스킬 로더 (SCI 기반).
   - 바이트코드 생성 없이 AST 인터프리팅으로 익명 함수 및 룰 즉시 마운트
   - sci/eval-string 시점에 :bindings로 buddha-call 심볼을 직접 주입"
  (:require [sci.core :as sci]
            [lbrain.buddha-proxy :as bp]
            [clojure.string :as str]))

;; =============================================================================
;; 1. 동적 스킬 레지스트리 & SCI 컨텍스트
;; =============================================================================

(defonce ^:dynamic *skill-registry* (atom {}))

;; 평가 스코프에 직접 주입할 심볼 맵
(def ^:private eval-bindings
  {'buddha-call   bp/call-skill
   'buddha-status bp/get-status
   'buddha-read   bp/read-tensor
   'buddha-write  bp/write-tensor})

(def ^:dynamic *sci-ctx* (sci/init {}))

;; =============================================================================
;; 2. S-식 동적 컴파일 및 마운트
;; =============================================================================

(defn register-symbolic-skill
  "문자열 형태의 S-expression 클로저를 SCI로 즉시 평가하여 레지스트리에 마운트"
  [skill-name-str sexp-code-str]
  (try
    (let [clean-name (str/replace (str skill-name-str) #"^:" "")
          skill-key  (keyword clean-name)
          ;; :bindings 옵션으로 eval-string 스코프에 직접 함수 전달
          parsed-fn  (sci/eval-string sexp-code-str
                                      {:ctx *sci-ctx*
                                       :bindings eval-bindings})]
      (if (fn? parsed-fn)
        (do
          (swap! *skill-registry* assoc skill-key parsed-fn)
          (println (format "[YAHWEH/Clojure] Symbolic skill ':%s' compiled and mounted." clean-name))
          true)
        (do
          (println (format "[YAHWEH/Clojure][ERROR] Evaluated code is not a function: %s" sexp-code-str))
          false)))
    (catch Throwable e
      (println (format "[YAHWEH/Clojure][ERROR] Skill registration failed for '%s': %s"
                       skill-name-str (.getMessage e)))
      false)))

(defn unregister-symbolic-skill
  "등록된 동적 스킬 언마운트"
  [skill-name-str]
  (let [skill-key (keyword (str/replace (str skill-name-str) #"^:" ""))]
    (swap! *skill-registry* dissoc skill-key)
    (println (format "[YAHWEH/Clojure] Skill '%s' unmounted." skill-key))
    true))

(defn invoke-symbolic-skill
  "등록된 동적 스킬 실행"
  [skill-name-str & args]
  (let [skill-key (keyword (str/replace (str skill-name-str) #"^:" ""))]
    (if-let [f (get @*skill-registry* skill-key)]
      (apply f args)
      (throw (ex-info (str "Skill not found: " skill-key) {:skill skill-key})))))

(defn list-mounted-skills
  "현재 마운트된 모든 기호 스킬 키 목록 반환"
  []
  (keys @*skill-registry*))