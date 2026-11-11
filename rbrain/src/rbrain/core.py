# rbrain/src/rbrain/core.py
"""
BUDDHA Core Runtime Infrastructure (rbrain.core)
- Rust(pycl) FFI와 직결되는 CPython 전담 최상위 런타임 클래스
- L0 물리 공유 메모리 인-플레이스 바인딩
- 동적 Python 모듈 핫패칭 및 프록시 중재
"""
from typing import Dict, Any, List, Optional
import traceback
import torch

from rbrain.memory import (
    bind_raw_memory,
    is_bound,
    get_capacity,
    get_raw_address,
    read_tensor_slice,
    write_tensor_slice,
)
import rbrain.dynamic_loader as loader
from rbrain.buddha.ego import BuddhaEgo
from rbrain.yahweh_proxy import YahwehProxy

class BuddhaCore:
    _instance: Optional["BuddhaCore"] = None

    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super(BuddhaCore, cls).__new__(cls)
            cls._instance._initialized = False 
        return cls._instance
    
    def __init__(self, device: Optional[str] = None):
        if self._initialized:
            return
        self.device = device if device else ("cuda" if torch.cuda.is_available() else "cpu")
        self._bootstrapped: bool = False
        self._activation_count: int = 0

        self.ego = BuddhaEgo()
        self._initialized = True
        print(f"[BUDDHA/Python] BuddhaCore runtime initialized (Device: {self.device})")

    # =========================================================================
    # 1. L0 물리 공유 메모리 제어
    # =========================================================================

    def bind_arena(self, ptr_address: int, capacity: int) -> bool:
        try:
            bind_raw_memory(ptr_address, capacity)
            self._bootstrapped = is_bound()
            return self._bootstrapped
        except Exception as e:
            print(f"[BUDDHA/Python Error] bind_arena failed: {e}")
            traceback.print_exc()
            return False

    def get_status(self) -> Dict[str, Any]:
        bound = is_bound()
        modules = loader.list_modules()
        return {
            "engine": "BUDDHA",
            "device": self.device,
            "bootstrapped": self._bootstrapped,
            "activation_count": self._activation_count,
            "arena": {
                "bound": bound,
                "raw_address": f"0x{get_raw_address():X}" if bound else None,
                "slots": get_capacity() if bound else 0,
                "capacity_bytes": (get_capacity() * 4) if bound else 0,
            },
            "mounted_modules_count": len(modules),
            "mounted_modules": list(modules.keys()),
        }

    # =========================================================================
    # 2. 동적 모듈 제어 인프라
    # =========================================================================

    def mount_module(self, mod_name: str, code_str: str) -> bool:
        return loader.register_module(mod_name, code_str)

    def call_module(self, mod_name: str, *args, **kwargs) -> Any:
        return loader.call_module(mod_name, *args, **kwargs)

    def unmount_module(self, mod_name: str) -> bool:
        return loader.unregister_module(mod_name)

    # =========================================================================
    # 3. 통합 인프라 디스패처 (Dispatch)
    # =========================================================================

    def dispatch(self, opcode: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        if opcode == "status":
            return self.get_status()

        if opcode == "mount_module":
            mod_name = payload["module_name"]
            code_str = payload["code"]
            success = self.mount_module(mod_name, code_str)
            return {"status": "SUCCESS" if success else "FAILED", "module": mod_name}

        if opcode == "call_module":
            mod_name = payload["module_name"]
            args = payload.get("args", [])
            kwargs = payload.get("kwargs", {})
            if "arg" in payload and not args:
                args = [payload["arg"]]
            result = self.call_module(mod_name, *args, **kwargs)
            return {"module": mod_name, "result": result}

        if opcode == "unmount_module":
            mod_name = payload["module_name"]
            return {"status": "SUCCESS", "unmounted": self.unmount_module(mod_name)}

        if opcode == "read_tensor":
            if not is_bound():
                raise RuntimeError("[BUDDHA Error] L0 Shared Arena is not bound")
            offset = payload["offset"]
            length = payload["length"]
            slice_tensor = read_tensor_slice(offset, length)
            return {"offset": offset, "length": length, "data": slice_tensor.tolist()}

        if opcode == "write_tensor":
            if not is_bound():
                raise RuntimeError("[BUDDHA Error] L0 Shared Arena is not bound")
            offset = payload["offset"]
            data = payload["data"]
            tensor_data = torch.tensor(data, dtype=torch.float32)
            written = write_tensor_slice(offset, tensor_data)
            return {"offset": offset, "written_slots": written}

        raise ValueError(f"[BUDDHA Error] Unsupported opcode: '{opcode}'")


# Rust 런타임에서 모듈 로드(import) 시 즉시 인스턴스 자동 생성
buddha_instance: BuddhaCore = BuddhaCore()


def dispatch(opcode: str, payload: Dict[str, Any]) -> Dict[str, Any]:
    """Rust PyO3에서 진입하는 최상위 모듈 디스패치 진입점"""
    return buddha_instance.dispatch(opcode, payload)