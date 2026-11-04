(ns lbrain.primitives.type-hierarchy
  (:refer-clojure :exclude [==])
  (:require [clojure.core.logic :refer :all]
            [clojure.string :as str]))

;; =============================================================================
;; 1. 직계 부모-자식 관계 사실 테이블 (Base Ontology Facts)
;; =============================================================================

;; 상위-하위 개념 공리계 팩트 (단일 통과 Base Model)
(def ^:private hierarchy-facts
  #{[:개념 :엔티티]
    [:엔티티 :최상위]
    [:인간 :영장류]
    [:영장류 :포유류]
    [:포유류 :동물]
    [:동물 :생명체]
    [:생명체 :실체]
    [:실체 :최상위]})

;; =============================================================================
;; 2. core.logic 관계 연산자 (순수 defn + conde 기반)
;; =============================================================================

;; 직계 포함 관계 (Direct Parent)
(defn direct-parento
  "child가 parent의 직계 하위 개념인지를 검증하는 Relational Goal"
  [child parent]
  (conde
    [(== child :개념) (== parent :엔티티)]
    [(== child :엔티티) (== parent :최상위)]
    [(== child :인간) (== parent :영장류)]
    [(== child :영장류) (== parent :포유류)]
    [(== child :포유류) (== parent :동물)]
    [(== child :동물) (== parent :생명체)]
    [(== child :생명체) (== parent :실체)]
    [(== child :실체) (== parent :최상위)]))

;; 이행적 포함 관계 (A ⊂ ... ⊂ B)
(defn ancestoro
  "child가 parent의 (재귀적) 하위 개념인지를 증명하는 Relational Goal"
  [child parent]
  (conde
    ;; 직접 직계인 경우
    [(direct-parento child parent)]
    ;; 중간 매개 개념을 거쳐 이행적으로 포함되는 경우
    [(fresh [mid]
       (direct-parento child mid)
       (ancestoro mid parent))]))

;; =============================================================================
;; 3. 고수준 포함 (⊂, Inclusion) 인터페이스
;; =============================================================================

(defn verify-inclusion-symbols
  "sub-concept가 super-concept에 포함(⊂)되는지 기호 논리로 검증"
  [sub-sym super-sym]
  (if (= sub-sym super-sym)
    true
    (let [results (run 1 [q]
                    (ancestoro sub-sym super-sym)
                    (== q true))]
      (boolean (seq results)))))

(defn verify-inclusion
  "Rust/pycl Bridge에서 C-ABI / JNI를 통해 호출하는 문자열 진입점"
  [sub-str super-str]
  (try
    (let [s (keyword (str/replace (str sub-str) #"^:" ""))
          p (keyword (str/replace (str super-str) #"^:" ""))]
      (verify-inclusion-symbols s p))
    (catch Exception e
      (println "[YAHWEH/Clojure][ERROR] Hierarchy verification error:" (.getMessage e))
      false)))
;; 기존 verify-inclusion 아래에 추가
(defn subtype?
  "동적 스킬 및 타입 검증용 별칭 (sub가 super의 하위 타입인가?)"
  [sub super]
  (verify-inclusion sub super))