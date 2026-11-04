(ns lbrain.primitives.contradiction
  (:refer-clojure :exclude [==])
  (:require [clojure.core.logic :refer :all]
            [clojure.string :as str]))

;; 상호 배타 공리계 (배타적 경계)
(defn mutually-exclusiveo
  "두 개념이 상호 배타(모순) 관계인지 증명하는 Relational Goal"
  [a b]
  (conde
    [(== a :유한) (== b :무한)]
    [(== a :무한) (== b :유한)]
    [(== a :참) (== b :거짓)]
    [(== a :거짓) (== b :참)]
    [(== a :존재) (== b :비존재)]
    [(== a :비존재) (== b :존재)]
    [(== a :물질) (== b :반물질)]
    [(== a :반물질) (== b :물질)]))

(defn- pairwise-contradiction? [t1 t2]
  (let [res (run 1 [q]
              (mutually-exclusiveo t1 t2)
              (== q true))]
    (boolean (seq res))))

(defn has-contradiction?
  "주어진 특성 컬렉션 내에 공리적 모순이 존재하는지 여부 반환"
  [traits]
  (try
    (let [symbols (mapv #(keyword (str/replace (str %) #"^:" "")) traits)
          n (count symbols)]
      (boolean
        (some true?
              (for [i (range n)
                    j (range (inc i) n)]
                (pairwise-contradiction? (nth symbols i) (nth symbols j))))))
    (catch Exception e
      (println "[YAHWEH/Clojure][ERROR] Contradiction check failed:" (.getMessage e))
      false)))

(defn verify-opposition
  "특성 집합 내에 상호 배타적 모순이 존재하는지 검증 (모순 감지 시 false 반환 -> 엣지 절단)"
  [traits]
  (not (has-contradiction? traits)))