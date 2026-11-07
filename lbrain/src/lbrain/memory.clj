;; swh-framework/lbrain/src/lbrain/memory.clj
(ns lbrain.memory
  "swhAI YAHWEH L0 Shared Memory Arena (DirectByteBuffer) 제로카피 브로커.
   - Rust(COSMOS)가 할당한 물리 메모리 버퍼(*mut f32)를 JNI DirectByteBuffer로 바인딩.
   - CPython(BUDDHA NumPy ndarray)과 단 1바이트의 복사 없이 동일한 가상 메모리 주소를 공유.
   - Universal Primitives(박스 임베딩, 텐서 슬라이스)의 기호적 인덱싱 및 직접 I/O 제어."
  (:import [java.nio ByteBuffer ByteOrder FloatBuffer]))

;; -----------------------------------------------------------------------------
;; 1. 공유 메모리 버퍼 상태 (Atomic State)
;; -----------------------------------------------------------------------------

(defonce ^:dynamic *shared-byte-buffer* (atom nil))
(defonce ^:dynamic *shared-float-buffer* (atom nil))
(defonce ^:dynamic *arena-capacity-floats* (atom 0))

(defn is-bound?
  "L0 물리 메모리 아레나가 정상 매핑되어 있는지 확인"
  []
  (some? @*shared-float-buffer*))

(defn get-capacity
  "바인딩된 아레나의 총 f32 슬롯 수 반환"
  []
  @*arena-capacity-floats*)

;; -----------------------------------------------------------------------------
;; 2. 제로카피 바인딩 엔드포인트 (JNI FFI Link)
;; -----------------------------------------------------------------------------

(defn bind-direct-buffer!
  "Rust 호스트(pycl/bridge/lbrain_bridge.rs)에서 NewDirectByteBuffer로 전달된
   물리 메모리 ByteBuffer를 네이티브 바이트 순서의 FloatBuffer로 래핑하여 등록."
  [^ByteBuffer byte-buffer]
  (if (nil? byte-buffer)
    (throw (IllegalArgumentException. "[YAHWEH/Clojure Error] Cannot bind null ByteBuffer"))
    (let [^FloatBuffer float-buf (-> byte-buffer
                                     (.order (ByteOrder/nativeOrder))
                                     (.asFloatBuffer))
          capacity (.capacity float-buf)]
      (reset! *shared-byte-buffer* byte-buffer)
      (reset! *shared-float-buffer* float-buf)
      (reset! *arena-capacity-floats* capacity)
      (println (format "[YAHWEH/Clojure] L0 Shared Arena memory mapped: %d slots (%d MB) [Zero-Copy Active]"
                       capacity
                       (quot (* capacity 4) (* 1024 1024))))
      0)))

;; -----------------------------------------------------------------------------
;; 3. 고속 원자적 I/O (Direct Memory Access)
;; -----------------------------------------------------------------------------

(defn get-float
  "L0 아레나의 절대 인덱스에서 f32 단일 값 직접 읽기"
  [^long index]
  (if-let [^FloatBuffer buf @*shared-float-buffer*]
    (if (or (< index 0) (>= index @*arena-capacity-floats*))
      (throw (IndexOutOfBoundsException. (str "Index out of bounds: " index)))
      (.get buf (int index)))
    (throw (IllegalStateException. "[YAHWEH/Clojure Error] Shared Arena is not mapped"))))

(defn set-float!
  "L0 아레나의 절대 인덱스에 f32 단일 값 직접 기록 (Python 및 Rust에 즉시 반영)"
  [^long index ^double val]
  (if-let [^FloatBuffer buf @*shared-float-buffer*]
    (if (or (< index 0) (>= index @*arena-capacity-floats*))
      (throw (IndexOutOfBoundsException. (str "Index out of bounds: " index)))
      (do
        (.put buf (int index) (float val))
        val))
    (throw (IllegalStateException. "[YAHWEH/Clojure Error] Shared Arena is not mapped"))))

;; -----------------------------------------------------------------------------
;; 4. 슬라이스(Slice) 및 텐서 벡터 추출 (Tensor Inter-Op)
;; -----------------------------------------------------------------------------

(defn read-slice
  "특정 오프셋(offset)부터 길이(len)만큼의 슬라이스를 float 배열 또는 시퀀스로 추출"
  [^long offset ^long len]
  (if-let [^FloatBuffer buf @*shared-float-buffer*]
    (if (or (< offset 0) (> (+ offset len) @*arena-capacity-floats*))
      (throw (IndexOutOfBoundsException. (format "Slice range [%d, %d] exceeds capacity %d"
                                                 offset (+ offset len) @*arena-capacity-floats*)))
      (let [dst (float-array len)]
        ;; 복사 없이 position 이동 후 벌크 get 수행 (스레드 안전성을 위해 duplicate)
        (let [^FloatBuffer dup (.duplicate buf)]
          (.position dup (int offset))
          (.get dup dst 0 (int len)))
        (vec dst)))
    (throw (IllegalStateException. "[YAHWEH/Clojure Error] Shared Arena is not mapped"))))

(defn write-slice!
  "Clojure 시퀀스 데이터를 L0 아레나의 특정 오프셋에 벌크 기록"
  [^long offset float-seq]
  (if-let [^FloatBuffer buf @*shared-float-buffer*]
    (let [len (count float-seq)]
      (if (or (< offset 0) (> (+ offset len) @*arena-capacity-floats*))
        (throw (IndexOutOfBoundsException. (format "Write range [%d, %d] exceeds capacity %d"
                                                   offset (+ offset len) @*arena-capacity-floats*)))
        (let [src (float-array float-seq)
              ^FloatBuffer dup (.duplicate buf)]
          (.position dup (int offset))
          (.put dup src 0 (int len))
          len)))
    (throw (IllegalStateException. "[YAHWEH/Clojure Error] Shared Arena is not mapped"))))

;; -----------------------------------------------------------------------------
;; 5. 6대 대수 프리미티브 박스 임베딩 헬퍼
;; -----------------------------------------------------------------------------

(defn read-box-embedding
  "포함(⊂) 연산용 박스 임베딩 [min_vec(dim), max_vec(dim)]을 읽어 구조화된 맵으로 반환"
  [^long offset ^long dim]
  (let [total-len (* 2 dim)
        data (read-slice offset total-len)]
    {:dim dim
     :min-vec (subvec data 0 dim)
     :max-vec (subvec data dim total-len)}))

(defn write-box-embedding!
  "개념 박스 임베딩 [min-vec, max-vec]을 L0 메모리에 기록"
  [^long offset min-vec max-vec]
  (let [dim (count min-vec)]
    (if (not= dim (count max-vec))
      (throw (IllegalArgumentException. "min-vec and max-vec must have identical dimensions"))
      (do
        (write-slice! offset min-vec)
        (write-slice! (+ offset dim) max-vec)
        (* 2 dim)))))
