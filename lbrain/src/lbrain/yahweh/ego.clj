;; lbrain/src/lbrain/yahweh/ego.clj
(ns lbrain.yahweh.ego
  "YAHWEH Ego Loop (lbrain.yahweh.ego)
   - 멈춤 없이 0부터 recur로 회전하며 내부 기호 인과 시간(ticktok)을 생성하는 자아 루프")

(defprotocol IYahwehEgo
  (get-ticktok [this])
  (stop-ego! [this]))

(defrecord YahwehEgo [ticktok-atom running-atom worker-thread]
  IYahwehEgo
  (get-ticktok [_]
    @ticktok-atom)
  (stop-ego! [_]
    (reset! running-atom false)))

(defn start-yahweh-ego
  "0부터 멈춤 없이 recur로 단조 증가하며 ticktok을 계속 콘솔에 출력하는 데몬 스레드 루프"
  ([] (start-yahweh-ego 500))
  ([sleep-ms]
   (let [ticktok-atom (atom 0)
         running-atom (atom true)
         worker (doto (Thread.
                       (fn []
                         (println "[YAHWEH/Clojure] YahwehEgo loop online. Autonomous ticktok stream initiated.")
                         (flush)
                         (loop []
                           (when @running-atom
                             (let [curr (swap! ticktok-atom inc)]
                               (println (format "[YAHWEH/Ego] ticktok: %d" curr))
                               (flush)
                               (Thread/sleep sleep-ms)
                               (recur))))))
                  (.setName "YahwehEgoWorker")
                  (.setDaemon true)
                  (.start))]
     (->YahwehEgo ticktok-atom running-atom worker))))

;; 네임스페이스 로드 시 즉시 데몬 루프 자동 가동
(defonce ego-instance
  (start-yahweh-ego 500))