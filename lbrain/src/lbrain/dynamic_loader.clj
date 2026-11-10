;; lbrain/src/lbrain/dynamic_loader.clj
(ns lbrain.dynamic-loader
  "swhAI YAHWEH 런타임 동적 S-식 로더 (SCI 기반 격리 실행 엔진)"
  (:require [sci.core :as sci]
            [lbrain.buddha-proxy :as bp]
            [clojure.string :as str]))

(defonce ^:dynamic *skill-registry* (atom {}))

;; 동적으로 마운트되는 코드에 노출할 인프라 API 바인딩
(def ^:private eval-bindings
  {'buddha-call   bp/call-module
   'buddha-status bp/get-status
   'buddha-read   bp/read-tensor
   'buddha-write  bp/write-tensor})

(def ^:dynamic *sci-ctx* (sci/init {}))

(defn register-symbolic-skill
  "문자열 형태의 S-식 함수를 런타임에 평가하여 레지스트리에 마운트"
  [skill-name-str sexp-code-str]
  (try
    (let [clean-name (str/replace (str skill-name-str) #"^:" "")
          skill-key  (keyword clean-name)
          parsed-fn  (sci/eval-string sexp-code-str
                                      {:ctx *sci-ctx*
                                       :bindings eval-bindings})]
      (if (fn? parsed-fn)
        (do
          (swap! *skill-registry* assoc skill-key parsed-fn)
          (println (format "[YAHWEH/Infrastructure] Dynamic module ':%s' mounted." clean-name))
          true)
        (do
          (println (format "[YAHWEH/Error] Evaluated code is not a function: %s" sexp-code-str))
          false)))
    (catch Throwable e
      (println (format "[YAHWEH/Error] Dynamic compile failed for '%s': %s"
                       skill-name-str (.getMessage e)))
      false)))

(defn unregister-symbolic-skill
  "마운트된 동적 모듈 언마운트"
  [skill-name-str]
  (let [skill-key (keyword (str/replace (str skill-name-str) #"^:" ""))]
    (swap! *skill-registry* dissoc skill-key)
    true))

(defn invoke-symbolic-skill
  "마운트된 동적 모듈 실행"
  [skill-name-str & args]
  (let [skill-key (keyword (str/replace (str skill-name-str) #"^:" ""))]
    (if-let [f (get @*skill-registry* skill-key)]
      (apply f args)
      (throw (ex-info (str "Module not found: " skill-key) {:module skill-key})))))

(defn list-mounted-skills
  "현재 마운트된 모듈 키 목록 반환"
  []
  (keys @*skill-registry*))