;; lbrain/src/lbrain/core.clj
(ns lbrain.core
  (:require [lbrain.memory :as mem]
            [lbrain.dynamic-loader :as dyn]
            [lbrain.buddha-proxy :as bp]
            [clojure.data.json :as json]))

(defprotocol IYahwehEngine
  (get-status [this])
  (bind-arena [this byte-buffer])
  (dispatch-op [this opcode payload]))

(defrecord YahwehInstance [engine-name backend-type state-atom]
  IYahwehEngine
  (get-status [_]
    (let [bound? (mem/is-bound?)
          modules (mapv str (dyn/list-mounted-skills))
          state @state-atom]
      {:engine engine-name
       :backend backend-type
       :bootstrapped (:bootstrapped state)
       :axiom_count (count (:axioms state))
       :arena {:bound bound?
               :slots (mem/get-capacity-slots)
               :capacity_bytes (mem/get-capacity-bytes)}
       :mounted_modules_count (count modules)}))

  (bind-arena [_ byte-buffer]
    (let [res (mem/bind-raw-memory byte-buffer)]
      (swap! state-atom assoc :bootstrapped res)
      res))

  (dispatch-op [this opcode payload]
    (case opcode
      "status"
      (get-status this)

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

      {:error (str "[YAHWEH Engine Error] Unsupported opcode: " opcode)})))

(defn- make-yahweh-instance []
  (let [backend (if (System/getProperty "java.vm.name") "graalvm-dev" "native-c-abi")]
    (->YahwehInstance
     "YAHWEH"
     backend
     (atom {:bootstrapped false
            :call-count 0
            :axioms #{[:axiom :identity :self-consistent]}}))))

;; Rust에서 require 되자마자 즉시 인스턴스 자동 생성 및 상시 활성화
(defonce yahweh-runtime
  (let [inst (make-yahweh-instance)]
    (println "[YAHWEH/Clojure] YahwehInstance auto-created and activated on module load.")
    inst))

(defn bind-shared-arena
  [^java.nio.ByteBuffer byte-buffer]
  (bind-arena yahweh-runtime byte-buffer))

(defn dispatch
  [^String opcode ^String payload-json]
  (let [payload (try
                  (json/read-str payload-json :key-fn keyword)
                  (catch Exception _ {}))]
    (json/write-str (dispatch-op yahweh-runtime opcode payload))))