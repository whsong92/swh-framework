# rbrain/src/rbrain/yahweh_proxy.py
"""
BUDDHA -> YAHWEH Reverse-FFI Proxy (Python side)
- Rust 호스트로부터 함수 포인터를 주입받아 Clojure 인스턴스 직접 호출
"""
import ctypes
import json
from typing import Any, Dict

_CALL_YAHWEH_FN = None
_STRING_FREE_FN = None


def _bind_host_symbols(call_yahweh_addr: int, string_free_addr: int) -> None:
    global _CALL_YAHWEH_FN, _STRING_FREE_FN
    c_call_proto = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p)
    _CALL_YAHWEH_FN = c_call_proto(call_yahweh_addr)

    c_free_proto = ctypes.CFUNCTYPE(None, ctypes.c_void_p)
    _STRING_FREE_FN = c_free_proto(string_free_addr)


class YahwehProxy:
    @classmethod
    def is_available(cls) -> bool:
        return _CALL_YAHWEH_FN is not None and _STRING_FREE_FN is not None

    @classmethod
    def dispatch_raw(cls, opcode: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        if not cls.is_available():
            raise RuntimeError("[BUDDHA Error] pycl C-ABI (swh_call_yahweh) is not available")

        op_bytes = opcode.encode("utf-8")
        payload_bytes = json.dumps(payload).encode("utf-8")

        raw_ptr = _CALL_YAHWEH_FN(op_bytes, payload_bytes)
        if not raw_ptr:
            raise RuntimeError(f"[BUDDHA Error] Null pointer from YAHWEH dispatch '{opcode}'")

        try:
            res_str = ctypes.cast(raw_ptr, ctypes.c_char_p).value.decode("utf-8")
            return json.loads(res_str)
        finally:
            _STRING_FREE_FN(raw_ptr)

    @classmethod
    def get_status(cls) -> Dict[str, Any]:
        return cls.dispatch_raw("status", {})

    @classmethod
    def mount_module(cls, module_name: str, sexp_code: str) -> bool:
        res = cls.dispatch_raw("mount_module", {"module_name": module_name, "code": sexp_code})
        return res.get("status") == "SUCCESS"

    @classmethod
    def call_module(cls, module_name: str, *args) -> Any:
        res = cls.dispatch_raw("call_module", {"module_name": module_name, "args": list(args)})
        return res.get("result")