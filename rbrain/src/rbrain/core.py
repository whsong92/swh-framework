# rbrain/src/rbrain/core.py
"""
BUDDHA Core Runtime Infrastructure (rbrain.core)
- Rust(pycl) FFI와 직결되는 CPython 전담 최상위 런타임 클래스
- L0 물리 공유 메모리 인-플레이스 바인딩
- 동적 Python 모듈 핫패칭 및 프록시 중재
"""
from typing import Dict, Any
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


class BuddhaCore:
    def __init__(self):
        self.device = "cuda" if torch.cuda.is_available() else "cpu"
        print(f"[BUDDHA/Python] BuddhaCore runtime initialized (Device: {self.device})")

    # =========================================================================
    # 1. L0 물리 공유 메모리 제어
    # =========================================================================

    def bind_arena(self, ptr_address: int, capacity: int) -> bool:
        try:
            bind_raw_memory(ptr_address, capacity)
            return is_bound()
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