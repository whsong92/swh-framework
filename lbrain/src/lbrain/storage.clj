;; swh-framework/lbrain/src/lbrain/storage.clj
(ns lbrain.storage
  "swhAI YAHWEH L1/L2 영속성 스토리지 인터페이스.
   - L1 Working Memory (Redis): 빠른 상태 벡터, 동기화 락, 단기 작업 기억.
   - L2 Cognitive Topology (SurrealDB): 불변 온톨로지 그래프, 인과 엣지, 토폴로지 이력.
   - Rust(pycl) 호스트 브리지(JNI/C-ABI)와 결합하며, 오프라인 시 인메모리 격리 Mock 지원."
  (:require [clojure.data.json :as json]
            [clojure.string :as str]))

;; -----------------------------------------------------------------------------
;; 1. 드라이버 브리지 어댑터 및 인메모리 폴백 저장소
;; -----------------------------------------------------------------------------

;; 외부 Rust 스토리지 매니저와의 연동 핸들러 (atom)
(defonce ^:dynamic *storage-bridge*
  (atom {:redis-set-fn nil
         :redis-get-fn nil
         :redis-del-fn nil
         :surreal-query-fn nil}))

;; 오프라인 개발 및 브리지 미연결 시 사용할 격리된 인메모리 스토어
(defonce ^:dynamic *mock-l1-store* (atom {}))
(defonce ^:dynamic *mock-l2-store* (atom {}))

(defn register-storage-bridge!
  "Rust 호스트(pycl)의 스토리지 함수 인터페이스를 바인딩"
  [{:keys [redis-set redis-get redis-del surreal-query]}]
  (swap! *storage-bridge* assoc
         :redis-set-fn redis-set
         :redis-get-fn redis-get
         :redis-del-fn redis-del
         :surreal-query-fn surreal-query)
  (println "[YAHWEH/Clojure] Storage driver bridge successfully mounted."))

;; -----------------------------------------------------------------------------
;; 2. L1 Fast Working Memory (Redis 추상화)
;; -----------------------------------------------------------------------------

(defn redis-set!
  "L1 Redis에 키-값(문자열/JSON) 저장"
  [^String k ^String v]
  (if-let [f (:redis-set-fn @*storage-bridge*)]
    (try
      (f k v)
      (catch Throwable e
        (println (format "[YAHWEH/Storage Error] L1 Redis SET failed: %s" (.getMessage e)))
        false))
    ;; Mock Fallback
    (do
      (swap! *mock-l1-store* assoc k v)
      true)))

(defn redis-get
  "L1 Redis에서 키 조회"
  [^String k]
  (if-let [f (:redis-get-fn @*storage-bridge*)]
    (try
      (f k)
      (catch Throwable e
        (println (format "[YAHWEH/Storage Error] L1 Redis GET failed: %s" (.getMessage e)))
        nil))
    ;; Mock Fallback
    (get @*mock-l1-store* k)))

(defn redis-del!
  "L1 Redis 키 삭제"
  [^String k]
  (if-let [f (:redis-del-fn @*storage-bridge*)]
    (try
      (f k)
      (catch Throwable e
        (println (format "[YAHWEH/Storage Error] L1 Redis DEL failed: %s" (.getMessage e)))
        false))
    ;; Mock Fallback
    (do
      (swap! *mock-l1-store* dissoc k)
      true)))

(defn save-working-state!
  "Clojure 데이터 구조를 JSON으로 직렬화하여 L1 작업 기억에 보관"
  [^String k state-data]
  (let [json-str (json/write-str state-data)]
    (redis-set! k json-str)))

(defn load-working-state
  "L1 작업 기억에서 JSON을 읽어 Clojure 데이터 구조로 역직렬화"
  [^String k]
  (when-let [raw (redis-get k)]
    (try
      (json/read-str raw :key-fn keyword)
      (catch Throwable e
        (println (format "[YAHWEH/Storage Error] JSON parse failed for key %s: %s" k (.getMessage e)))
        nil))))

;; -----------------------------------------------------------------------------
;; 3. L2 Cognitive Topology & Ontology (SurrealDB 추상화)
;; -----------------------------------------------------------------------------

(defn surreal-query!
  "L2 SurrealDB에 SurrealQL 쿼리를 발행하고 결과를 Clojure 데이터 구조로 반환"
  [^String sql]
  (if-let [f (:surreal-query-fn @*storage-bridge*)]
    (try
      (let [raw (f sql)]
        (if (string? raw)
          (json/read-str raw :key-fn keyword)
          raw))
      (catch Throwable e
        (println (format "[YAHWEH/Storage Error] L2 Surreal query failed: %s" (.getMessage e)))
        {:error (.getMessage e)}))
    ;; Mock Fallback: 쿼리 패턴 단순 로깅 및 Mock 응답
    (do
      (println (format "[YAHWEH/Storage Mock] Executed SurrealQL: %s" sql))
      {:status "MOCK_OK" :query sql})))

(defn record-ontology-node!
  "L2 지식 그래프에 개념 노드 저장 (테이블: concept_node)"
  [node-id concept-type traits]
  (let [node-data {:id node-id
                   :concept_type (name concept-type)
                   :traits (map name traits)}
        sql (format "UPSERT concept_node:%s CONTENT %s;"
                    node-id
                    (json/write-str node-data))]
    (surreal-query! sql)))

(defn record-causal-edge!
  "L2 지식 그래프에 인과 및 대수적 관계 엣지 기록 (테이블: topology_edge)"
  [from-id relation to-id weight]
  (let [edge-id (format "%s_%s_%s" from-id (name relation) to-id)
        edge-data {:from from-id
                   :to to-id
                   :primitive (name relation)
                   :weight (float weight)}
        sql (format "UPSERT topology_edge:%s CONTENT %s;"
                    edge-id
                    (json/write-str edge-data))]
    (surreal-query! sql)))

;; -----------------------------------------------------------------------------
;; 4. 영속성 계층 진단 및 검증 인터페이스
;; -----------------------------------------------------------------------------

(defn verify
  "스토리지 시스템 정상 작동 검증"
  []
  (println "=== [YAHWEH/Clojure] Storage System Verification ===")
  (redis-set! "yahweh:status" "RULE_ENGINE_ACTIVE")
  (let [status-val (redis-get "yahweh:status")]
    (println "[YAHWEH/Clojure] L1 Fast Working Memory Read:" status-val))
  (let [res (surreal-query! "INFO FOR DB;")]
    (println "[YAHWEH/Clojure] L2 Cognitive Topology Graph Info:" res))
  0)