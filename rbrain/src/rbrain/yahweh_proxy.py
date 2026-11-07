# rbrain/src/rbrain/yahweh_proxy.py
"""
swhAI BUDDHA -> YAHWEH Reverse-FFI Proxy (Python side)
- Rust 호스트 바이너리로부터 C-ABI 함수 포인터를 직접 주입받아 직결 바인딩
- 외부 .so 파일 재로딩으로 인한 GLOBAL_ROUTER 미등록/격리 버그 원천 차단
"""

import ctypes
import json
import os
from typing import Any, Dict, List, Optional

_CALL_YAHWEH_FN = None
_STRING_FREE_FN = None


def _bind_host_symbols(call_yahweh_addr: int, string_free_addr: int) -> None:
    """Rust 호스트(rbrain_bridge.rs)에서 기동 시 호출하여 메모리 주소를 직접 바인딩"""
    global _CALL_YAHWEH_FN, _STRING_FREE_FN

    c_call_proto = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p)
    _CALL_YAHWEH_FN = c_call_proto(call_yahweh_addr)

    c_free_proto = ctypes.CFUNCTYPE(None, ctypes.c_void_p)
    _STRING_FREE_FN = c_free_proto(string_free_addr)


def _ensure_symbols() -> None:
    """호스트에서 주입되지 않은 독립 테스트 환경일 경우의 로컬 라이브러리 fallback"""
    global _CALL_YAHWEH_FN, _STRING_FREE_FN
    if _CALL_YAHWEH_FN is not None:
        return

    # 1. 인-프로세스 전역 심볼 테이블(RTLD_DEFAULT) 검색
    try:
        lib = ctypes.CDLL(None)
        if hasattr(lib, "swh_call_yahweh"):
            _CALL_YAHWEH_FN = lib.swh_call_yahweh
            _CALL_YAHWEH_FN.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
            _CALL_YAHWEH_FN.restype = ctypes.c_void_p

            _STRING_FREE_FN = lib.swh_string_free
            _STRING_FREE_FN.argtypes = [ctypes.c_void_p]
            _STRING_FREE_FN.restype = None
            return
    except Exception:
        pass

    # 2. 로컬 빌드 아티팩트 검색
    candidate_paths = [
        "target/debug/libpycl.so",
        "../target/debug/libpycl.so",
        "../../target/debug/libpycl.so",
        "target/release/libpycl.so",
        "../target/release/libpycl.so",
    ]
    for path in candidate_paths:
        if os.path.exists(path):
            try:
                lib = ctypes.CDLL(os.path.abspath(path))
                if hasattr(lib, "swh_call_yahweh"):
                    _CALL_YAHWEH_FN = lib.swh_call_yahweh
                    _CALL_YAHWEH_FN.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
                    _CALL_YAHWEH_FN.restype = ctypes.c_void_p

                    _STRING_FREE_FN = lib.swh_string_free
                    _STRING_FREE_FN.argtypes = [ctypes.c_void_p]
                    _STRING_FREE_FN.restype = None
                    return
            except Exception:
                continue


class YahwehProxy:
    """Python에서 Clojure(YAHWEH) 기호 엔진을 제어하는 프록시 클래스"""

    @classmethod
    def is_available(cls) -> bool:
        _ensure_symbols()
        return _CALL_YAHWEH_FN is not None and _STRING_FREE_FN is not None

    @classmethod
    def dispatch_raw(cls, opcode: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        if not cls.is_available():
            raise RuntimeError("[BUDDHA Error] pycl C-ABI (swh_call_yahweh) is not available")

        op_bytes = opcode.encode("utf-8")
        payload_bytes = json.dumps(payload).encode("utf-8")

        raw_ptr = _CALL_YAHWEH_FN(op_bytes, payload_bytes)
        if not raw_ptr:
            raise RuntimeError(f"[BUDDHA Error] Null pointer returned from YAHWEH dispatch '{opcode}'")

        try:
            res_str = ctypes.cast(raw_ptr, ctypes.c_char_p).value.decode("utf-8")
            return json.loads(res_str)
        finally:
            _STRING_FREE_FN(raw_ptr)

    @classmethod
    def get_status(cls) -> Dict[str, Any]:
        """Clojure 런타임 상태 및 마운트된 스킬 목록 조회"""
        return cls.dispatch_raw("status", {})

    @classmethod
    def mount_skill(cls, skill_name: str, clojure_sexp_code: str) -> bool:
        """Clojure 측에 동적 S-식 스킬 실시간 컴파일 및 마운트"""
        res = cls.dispatch_raw("mount_skill", {
            "skill_name": skill_name,
            "code": clojure_sexp_code,
        })
        return res.get("status") == "SUCCESS"

    @classmethod
    def call_skill(cls, skill_name: str, arg: Any) -> Any:
        """Clojure 측에 마운트된 동적 스킬 실행"""
        res = cls.dispatch_raw("dynamic_call", {
            "skill_name": skill_name,
            "arg": arg,
        })
        return res.get("result")

    @classmethod
    def causal_step(cls, state: Dict[str, Any], action: Dict[str, Any]) -> Dict[str, Any]:
        """Clojure의 YAHWEH 인과 전이 1단계 수행 (모순 감지 시 Pruned)"""
        return cls.dispatch_raw("causal_step", {
            "state": state,
            "action": action,
        })

    @classmethod
    def causal_pipeline(cls, init_traits: List[str], actions: List[Dict[str, Any]]) -> Dict[str, Any]:
        """초기 특성 집합과 액션 시퀀스를 일괄 시뮬레이션"""
        return cls.dispatch_raw("causal_pipeline", {
            "init": init_traits,
            "actions": actions,
        })