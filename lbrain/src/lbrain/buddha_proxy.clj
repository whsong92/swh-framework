;; lbrain/src/lbrain/buddha_proxy.clj
(ns lbrain.buddha-proxy
  "swhAI YAHWEH -> BUDDHA Reverse-FFI Proxy (Clojure side)
   - Rust 호스트 바이너리로부터 함수 포인터를 직접 주입받아 바인딩
   - .so 중복 로딩으로 인한 GLOBAL_ROUTER 미등록 격리 문제 원천 차단"
  (:require [clojure.data.json :as json]
            [clojure.string :as str])
  (:import [com.sun.jna Pointer Function]))

;; -----------------------------------------------------------------------------
;; 1. C-ABI 함수 포인터 보관소 (Rust 호스트에서 직접 주입)
;; -----------------------------------------------------------------------------

(defonce ^:private host-symbols
  (atom {:call nil :free nil}))

(defn bind-host-symbols!
  "Rust lbrain_bridge.rs 부트스트랩 시 C-ABI 진입점 함수 포인터 직접 바인딩"
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

;; -----------------------------------------------------------------------------
;; 2. C-ABI Raw 디스패치 및 메모리 해제
;; -----------------------------------------------------------------------------

(defn dispatch-raw
  "Clojure에서 Python(BUDDHA)으로 단독 디스패치 동기 호출"
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

;; -----------------------------------------------------------------------------
;; 3. 고수준 편의 인터페이스 (API Wrappers)
;; -----------------------------------------------------------------------------

(defn get-status
  "Python(BUDDHA) 런타임 및 디바이스 상태 조회"
  []
  (dispatch-raw "status" {}))

(defn mount-skill
  "Python 측에 동적 파이썬/PyTorch 스킬을 실시간 마운트"
  [^String skill-name ^String python-code-str]
  (let [res (dispatch-raw "mount_skill" {:skill_name skill-name :code python-code-str})]
    (= (:status res) "SUCCESS")))

(defn call-skill
  "Python 측에 마운트된 동적 스킬 실행"
  [^String skill-name & args]
  (let [res (dispatch-raw "dynamic_call" {:skill_name skill-name :args (vec args)})]
    (:result res)))

(defn read-tensor
  "Python을 통해 L0 아레나 슬라이스 조회"
  [offset length]
  (dispatch-raw "read_tensor" {:offset offset :length length}))

(defn write-tensor
  "Python을 통해 L0 아레나 슬라이스 기록"
  [offset data-seq]
  (dispatch-raw "write_tensor" {:offset offset :data (vec data-seq)}))