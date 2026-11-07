;; lbrain/src/lbrain/memory.clj
(ns lbrain.memory
  (:import [java.nio ByteBuffer ByteOrder]))

;; L0 물리 공유 메모리 전역 참조 (DirectByteBuffer)
(defonce ^:dynamic *shared-arena* (atom nil))

(defn is-bound?
  "L0 물리 메모리 아레나 바인딩 여부 확인"
  []
  (some? @*shared-arena*))

(defn bind-raw-memory
  "Rust 호스트(pycl)에서 전달받은 DirectByteBuffer를 제로카피로 바인딩.
   (Python의 memory.py:bind_raw_memory와 1:1 대칭)"
  [^ByteBuffer byte-buffer]
  (when (nil? byte-buffer)
    (throw (IllegalArgumentException. "[YAHWEH/Clojure Error] ByteBuffer is nil")))
  (.order byte-buffer ByteOrder/LITTLE_ENDIAN)
  (reset! *shared-arena* byte-buffer)
  (println (format "[YAHWEH/Clojure] L0 DirectByteBuffer bound: %d bytes (Direct: %s)"
                   (.capacity byte-buffer)
                   (.isDirect byte-buffer)))
  true)

(defn get-capacity-bytes []
  (if-let [^ByteBuffer bb @*shared-arena*]
    (.capacity bb)
    0))

(defn get-capacity-slots []
  (quot (get-capacity-bytes) 4))

(defn get-float
  "L0 아레나의 특정 f32 슬롯 값 조회"
  [slot-idx]
  (if-let [^ByteBuffer bb @*shared-arena*]
    (let [byte-offset (* (int slot-idx) 4)]
      (if (<= (+ byte-offset 4) (.capacity bb))
        (.getFloat bb byte-offset)
        (throw (IndexOutOfBoundsException. (str "Slot index out of bounds: " slot-idx)))))
    (throw (IllegalStateException. "Shared memory is not bound"))))

(defn set-float!
  "L0 아레나의 특정 f32 슬롯에 f32 값 기록"
  [slot-idx ^double val]
  (if-let [^ByteBuffer bb @*shared-arena*]
    (let [byte-offset (* (int slot-idx) 4)]
      (if (<= (+ byte-offset 4) (.capacity bb))
        (do (.putFloat bb byte-offset (float val)) true)
        (throw (IndexOutOfBoundsException. (str "Slot index out of bounds: " slot-idx)))))
    (throw (IllegalStateException. "Shared memory is not bound"))))

(defn read-slice
  "지정된 슬롯 오프셋부터 길이만큼 f32 벡터로 읽기"
  [offset length]
  (let [off (int offset)
        len (int length)]
    (mapv #(get-float (+ off %)) (range len))))

(defn write-slice!
  "지정된 슬롯 오프셋부터 f32 시퀀스 데이터를 순차 기록"
  [offset vals]
  (let [off (int offset)]
    (doseq [[idx val] (map-indexed vector vals)]
      (set-float! (+ off idx) (double val)))
    (count vals)))