# rbrain/src/rbrain/core.py
"""
BUDDHA Core Neural Engine (rbrain.core)
- Rust 호스트(pycl)와 FFI로 연동되는 CPython 전담 최상위 런타임 제어 클래스.
- 정적 AI 연산자를 배제하고 L0 물리 메모리 바인딩 및 동적 스킬 핫패칭만 전담합니다.
"""
from typing import Dict, Any, Optional, Callable
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
        print(f"[BUDDHA/Python] BuddhaCore initialized on device: {self.device} (PyTorch {torch.__version__})")

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
        skills = loader.list_skills()
        return {
            "engine": "BUDDHA",
            "device": self.device,
            "torch_version": torch.__version__,
            "cuda_available": torch.cuda.is_available(),
            "arena": {
                "bound": bound,
                "raw_address": f"0x{get_raw_address():X}" if bound else None,
                "slots": get_capacity() if bound else 0,
                "capacity_bytes": (get_capacity() * 4) if bound else 0,
            },
            "mounted_skills_count": len(skills),
            "mounted_skills": list(skills.keys()),
        }

    # =========================================================================
    # 2. 동적 스킬 제어
    # =========================================================================

    def mount_dynamic_skill(self, skill_name: str, code_str: str) -> bool:
        return loader.register_skill(skill_name, code_str)

    def call_dynamic_skill(self, skill_name: str, *args, **kwargs) -> Any:
        return loader.call_skill(skill_name, *args, **kwargs)

    # =========================================================================
    # 3. 통합 디스패처 (Dispatch)
    # =========================================================================

    @torch.no_grad()
    def dispatch(self, opcode: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        if opcode == "status":
            return self.get_status()

        if opcode == "mount_skill":
            skill_name = payload["skill_name"]
            code_str = payload["code"]
            success = self.mount_dynamic_skill(skill_name, code_str)
            return {"status": "SUCCESS" if success else "FAILED", "skill": skill_name}

        if opcode in ("dynamic_call", "call_skill"):
            skill_name = payload["skill_name"]
            args = payload.get("args", [])
            kwargs = payload.get("kwargs", {})
            if "arg" in payload and not args:
                args = [payload["arg"]]
            result = self.call_dynamic_skill(skill_name, *args, **kwargs)
            return {"skill": skill_name, "result": result}

        if opcode == "read_tensor":
            if not is_bound():
                raise RuntimeError("[BUDDHA Core Error] L0 Shared Arena is not bound")
            offset = payload["offset"]
            length = payload["length"]
            slice_tensor = read_tensor_slice(offset, length)
            return {"offset": offset, "length": length, "data": slice_tensor.tolist()}

        if opcode == "write_tensor":
            if not is_bound():
                raise RuntimeError("[BUDDHA Core Error] L0 Shared Arena is not bound")
            offset = payload["offset"]
            data = payload["data"]
            tensor_data = torch.tensor(data, dtype=torch.float32)
            written = write_tensor_slice(offset, tensor_data)
            return {"offset": offset, "written_slots": written}

        raise ValueError(f"[BUDDHA Core Error] Unsupported opcode: '{opcode}'")