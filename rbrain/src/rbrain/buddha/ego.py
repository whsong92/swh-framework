# rbrain/src/rbrain/buddha/ego.py
"""
BUDDHA Ego Loop (rbrain.buddha.ego)
- 멈춤 없이 0부터 회전하며 내부 신경 인지 시간(ticktok)을 생성하는 자아 데몬 루프
"""
import threading
import time


class BuddhaEgo:
    def __init__(self):
        self.ticktok: int = 0
        self.is_running: bool = True
        self._thread = threading.Thread(
            target=self._run_loop,
            daemon=True,
            name="BuddhaEgoWorker"
        )
        self._thread.start()

    def _run_loop(self):
        print("[BUDDHA/Python] BuddhaEgo loop online. ticktok started from 0.")
        while self.is_running:
            self.ticktok += 1
            print(f"[BUDDHA/Ego] ticktok: {self.ticktok}", flush=True)
            # CPython GIL 양보 및 CPU 100% 폭주 방지 (1ms 간격)
            time.sleep(0.5)

    def get_ticktok(self) -> int:
        return self.ticktok

    def stop(self):
        self.is_running = False