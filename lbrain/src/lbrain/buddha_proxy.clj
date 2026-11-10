;; lbrain/src/lbrain/buddha_proxy.clj
(ns lbrain.buddha-proxy
  "swhAI YAHWEH -> BUDDHA Reverse-FFI Proxy (Clojure side)"
  (:require [clojure.data.json :as json]
            [clojure.string :as str])
  (:import [com.sun.jna Pointer Function]))

(defonce ^:private host-symbols
  (atom {:call nil :free nil}))

(defn bind-host-symbols!
  [^Long call-addr ^Long free-addr]
  (try
    (let [call-fn (Function/getFunction (Pointer. call-addr))
          free-fn (Function/getFunction (Pointer. free-addr))]
      (reset! host-symbols {:call call-fn :free free-fn})
      (println (format "[YAHWEH/Clojure] Bound host C-ABI symbols: call=0x%X, free=0x%X"
                       call-addr free-addr))
      true)
    (catch Throwable e
      (println "[YAHWEH/Warning] bind-host-symbols! failed:" (.getMessage e))
      false)))

(defn dispatch-raw
  [^String opcode payload-map]
  (let [{:keys [call free]} @host-symbols]
    (if-not (and call free)
      {:error "pycl C-ABI (swh_call_buddha) is not initialized"}
      (let [payload-json (json/write-str payload-map)
            ^Pointer raw-ptr (.invoke call Pointer (to-array [opcode payload-json]))]
        (if (nil? raw-ptr)
          {:error (str "Null response pointer from BUDDHA for opcode: " opcode)}
          (try
            (let [res-str (.getString raw-ptr 0 "UTF-8")]
              (try
                (json/read-str res-str :key-fn keyword)
                (catch Exception _
                  {:raw-result res-str})))
            (finally
              (.invoke free Void (to-array [raw-ptr])))))))))

(defn get-status []
  (dispatch-raw "status" {}))

(defn mount-module
  [^String module-name ^String python-code-str]
  (let [res (dispatch-raw "mount_module" {:module_name module-name :code python-code-str})]
    (= (:status res) "SUCCESS")))

(defn call-module
  [^String module-name & args]
  (let [res (dispatch-raw "call_module" {:module_name module-name :args (vec args)})]
    (:result res)))

(defn read-tensor [offset length]
  (dispatch-raw "read_tensor" {:offset offset :length length}))

(defn write-tensor [offset data-seq]
  (dispatch-raw "write_tensor" {:offset offset :data (vec data-seq)}))