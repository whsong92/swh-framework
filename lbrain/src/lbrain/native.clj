;; lbrain/src/lbrain/native.clj
(ns lbrain.native
  (:gen-class
    :name lbrain.native.YahwehNative
    :methods [^{:static true} [verify_memory_c [long long] int]
              ^{:static true} [bind_arena_c [long long] int]
              ^{:static true} [ping_c [] String]
              ^{:static true} [eval_sexp_c [String] String]])
  (:import [sun.misc Unsafe]))

(def ^Unsafe unsafe
  (try
    (let [field (.getDeclaredField Unsafe "theUnsafe")]
      (.setAccessible field true)
      (.get field nil))
    (catch Throwable e
      (println "[YAHWEH/Native Error] Failed to obtain Unsafe instance: " (.getMessage e))
      nil)))

(defonce native-arena-ptr (atom 0))
(defonce native-arena-size (atom 0))

(defn -bind_arena_c
  [^Long mem-ptr ^Long float-count]
  (try
    (if (or (zero? mem-ptr) (nil? unsafe))
      (do
        (println "[YAHWEH/Native Error] Invalid memory pointer or Unsafe unavailable.")
        (int -1))
      (do
        (reset! native-arena-ptr mem-ptr)
        (reset! native-arena-size float-count)
        (println (format "[YAHWEH/Native] Bound Raw Memory Pointer: 0x%X, Size: %d slots (%d MB) [Zero-Copy Native]"
                         mem-ptr float-count (quot (* float-count 4) (* 1024 1024))))
        (int 0)))
    (catch Throwable e
      (println "[YAHWEH/Native Error] bind_arena_c failed: " (.getMessage e))
      (int -2))))

(defn -verify_memory_c
  [^Long mem-ptr ^Long float-count]
  (try
    (if (or (zero? mem-ptr) (nil? unsafe))
      (int -1)
      (let [curr-val (.getFloat unsafe (long mem-ptr))
            test-token (float 777.0)]
        (println (format "[YAHWEH/Native] Verifying Arena Memory at 0x%X. Read val: %.2f" mem-ptr curr-val))
        (.putFloat unsafe (long mem-ptr) test-token)
        (println (format "[YAHWEH/Native] Verification Token %.2f written to 0x%X." test-token mem-ptr))
        (int 0)))
    (catch Throwable e
      (println "[YAHWEH/Native Error] verify_memory_c exception: " (.getMessage e))
      (int -1))))

(defn -ping_c []
  "YAHWEH Native AOT online (GraalVM native-image C-ABI)")

(defn -eval_sexp_c
  [^String sexp-str]
  (try
    (if (nil? sexp-str)
      "(:error \"Null S-expression\")"
      (let [trimmed (.trim sexp-str)
            res (eval (read-string trimmed))]
        (pr-str res)))
    (catch Throwable e
      (format "(:error \"%s\")" (.getMessage e)))))