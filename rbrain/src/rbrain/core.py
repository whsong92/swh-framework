"""
BUDDHA Core Neural Engine (rbrain.core)
Rust 호스트(pycl)와 FFI로 연동되는 CPython 전담 최상위 런타임 제어 클래스.
하드코딩된 대수/기하학 연산자를 배제하고 L0 물리 메모리 바인딩 및 동적 스킬 핫패칭을 전담합니다.
"""
from typing import Dict, Any, Optional, Tuple, Callable
import traceback
import torch
import numpy as np

from rbrain.memory import (
    bind_raw_memory,
    is_bound,
    get_capacity,
    get_raw_address,
    read_tensor_slice,
    write_tensor_slice,
)

# dynamic_loader 모듈의 다양한 네이밍 호환 처리
try:
    import rbrain.dynamic_loader as loader
    loader_mount_skill = getattr(loader, "mount_skill", getattr(loader, "register_dynamic_skill", None))
    loader_call_skill = getattr(loader, "call_skill", getattr(loader, "invoke_dynamic_skill", None))
    loader_list_skills = getattr(loader, "list_skills", getattr(loader, "list_mounted_skills", lambda: []))
except Exception:
    loader_mount_skill = None
    loader_call_skill = None
    loader_list_skills = lambda: []


class BuddhaCore:
    """
    BUDDHA 신경망 엔진 메인 진입점.
    Rust의 RbrainBridge 워커 스레드가 단일 인스턴스로 생명주기를 관리합니다.
    """

    def __init__(self):
        self.device = "cuda" if torch.cuda.is_available() else "cpu"
        self._local_registry: Dict[str, Callable] = {}
        print(f"[BUDDHA/Python] BuddhaCore initialized on device: {self.device} (PyTorch {torch.__version__})")

    # =========================================================================
    # 1. L0 물리 공유 메모리 제어
    # =========================================================================

    def bind_arena(self, ptr_address: int, capacity: int) -> bool:
        """
        Rust 호스트로부터 원시 포인터 주소(0x...)와 슬롯 크기를 받아
        L0 물리 메모리를 PyTorch/NumPy 제로카피 뷰로 바인딩합니다.
        """
        try:
            bind_raw_memory(ptr_address, capacity)
            return is_bound()
        except Exception as e:
            print(f"[BUDDHA/Python Error] bind_arena failed: {e}")
            traceback.print_exc()
            return False

    def get_status(self) -> Dict[str, Any]:
        """BUDDHA 런타임의 하드웨어, L0 메모리, 마운트된 스킬 상태를 디스크립터로 반환"""
        bound = is_bound()
        extra_skills = loader_list_skills() if callable(loader_list_skills) else []
        total_skills = list(set(list(self._local_registry.keys()) + list(extra_skills)))
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
            "mounted_skills_count": len(total_skills),
            "mounted_skills": total_skills,
        }

    # =========================================================================
    # 2. 런타임 동적 핫패칭 및 가소성 제어 (Dynamic Hot-Patching)
    # =========================================================================

    def mount_dynamic_skill(self, skill_name: str, code_str: str) -> bool:
        """
        동적 생성된 파이썬 코드를 런타임에 즉석 컴파일하여 레지스트리에 마운트합니다.
        """
        try:
            local_scope: Dict[str, Any] = {}
            global_scope = {
                "torch": torch,
                "np": np,
                "read_tensor_slice": read_tensor_slice,
                "write_tensor_slice": write_tensor_slice,
            }
            exec(code_str, global_scope, local_scope)

            target_fn = local_scope.get(skill_name) or next(
                (v for v in local_scope.values() if callable(v)), None
            )
            if target_fn is None:
                print(f"[BUDDHA/Python Error] No callable object found for skill '{skill_name}'")
                return False

            self._local_registry[skill_name] = target_fn
            if callable(loader_mount_skill):
                try:
                    loader_mount_skill(skill_name, target_fn)
                except Exception:
                    pass

            print(f"[BUDDHA/Python] Dynamic skill '{skill_name}' successfully compiled and mounted.")
            return True
        except Exception as e:
            print(f"[BUDDHA/Python Error] Failed to compile dynamic skill '{skill_name}': {e}")
            traceback.print_exc()
            return False

    def call_dynamic_skill(self, skill_name: str, *args, **kwargs) -> Any:
        """마운트된 동적 신경망 스킬 실행"""
        fn = self._local_registry.get(skill_name)
        if fn is not None:
            return fn(*args, **kwargs)
        if callable(loader_call_skill):
            return loader_call_skill(skill_name, *args, **kwargs)
        raise KeyError(f"[BUDDHA/Python Error] Skill '{skill_name}' is not mounted.")

    # =========================================================================
    # 3. 통합 디스패처 (Dispatch)
    # =========================================================================

    @torch.no_grad()
    def dispatch(self, opcode: str, payload: Dict[str, Any]) -> Dict[str, Any]:
        """
        Rust 호스트(pycl)에서 들어오는 모든 요청을 단일 진입점에서 분기 처리합니다.
        """
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