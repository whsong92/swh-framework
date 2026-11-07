# rbrain/src/rbrain/dynamic_loader.py
"""
swhAI BUDDHA 런타임 동적 스킬 로더 및 핫패칭 엔진
- types.ModuleType 기반 격리된 환경 내에서 파이썬/PyTorch 코드 동적 컴파일
- 고정된 AI 프리미티브 완전 배제: 순수 PyTorch/NumPy 및 L0 제로카피 메모리 배관만 주입
"""

from typing import Dict, Any, Optional, Callable
import types
import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

from rbrain.memory import (
    get_shared_arena,
    read_tensor_slice,
    write_tensor_slice,
    is_bound
)

_SKILL_REGISTRY: Dict[str, Dict[str, Any]] = {}


def _build_sandbox_environment(module_name: str) -> types.ModuleType:
    """동적 코드가 L0 물리 메모리와 텐서 라이브러리에 안전하게 접근할 수 있도록 샌드박스 구성"""
    mod = types.ModuleType(module_name)
    mod.__dict__.update({
        "__name__": module_name,
        "__doc__": f"swhAI BUDDHA Dynamic Isolated Module [{module_name}]",
        # 수치 / 텐서 프레임워크
        "torch": torch,
        "nn": nn,
        "F": F,
        "np": np,
        # L0 제로카피 공유 메모리 배관
        "get_shared_arena": get_shared_arena,
        "read_tensor_slice": read_tensor_slice,
        "write_tensor_slice": write_tensor_slice,
        "is_arena_bound": is_bound,
    })
    return mod


def register_skill(skill_name: str, code_str: str) -> bool:
    """전역 공간 오염 없이 독립된 types.ModuleType 안에서 코드를 컴파일하고 보관"""
    try:
        mod = _build_sandbox_environment(skill_name)
        exec(code_str, mod.__dict__)

        # 진입점(Callable) 탐색
        entrypoint: Optional[Callable] = getattr(mod, skill_name, None)
        if entrypoint is None:
            for fallback in ("forward", "run", "execute"):
                if hasattr(mod, fallback) and callable(getattr(mod, fallback)):
                    entrypoint = getattr(mod, fallback)
                    break

        if entrypoint is None:
            for attr_name, attr_val in mod.__dict__.items():
                if not attr_name.startswith("_") and (callable(attr_val) or isinstance(attr_val, nn.Module)):
                    entrypoint = attr_val
                    break

        if entrypoint is None:
            print(f"[BUDDHA/Python Error] No callable entrypoint found in skill '{skill_name}'")
            return False

        if isinstance(entrypoint, type) and issubclass(entrypoint, nn.Module):
            entrypoint = entrypoint()
            entrypoint.eval()

        current_ver = _SKILL_REGISTRY.get(skill_name, {}).get("version", 0) + 1
        _SKILL_REGISTRY[skill_name] = {
            "module": mod,
            "entrypoint": entrypoint,
            "source": code_str,
            "version": current_ver
        }

        print(f"[BUDDHA/Python] Dynamic skill '{skill_name}' (v{current_ver}) compiled and mounted successfully.")
        return True

    except Exception as e:
        print(f"[BUDDHA/Python Error] Failed to compile dynamic skill '{skill_name}': {e}")
        return False


def unregister_skill(skill_name: str) -> bool:
    """등록된 동적 스킬 언마운트"""
    if skill_name in _SKILL_REGISTRY:
        del _SKILL_REGISTRY[skill_name]
        print(f"[BUDDHA/Python] Skill '{skill_name}' unmounted.")
        return True
    return False


@torch.no_grad()
def call_skill(skill_name: str, *args, **kwargs) -> Any:
    """마운트된 동적 스킬 실행"""
    if skill_name not in _SKILL_REGISTRY:
        raise RuntimeError(f"[BUDDHA/Python Error] Skill '{skill_name}' is not registered.")

    entrypoint = _SKILL_REGISTRY[skill_name]["entrypoint"]
    try:
        result = entrypoint(*args, **kwargs)
        if isinstance(result, torch.Tensor):
            if result.numel() == 1:
                return float(result.item())
            return result.detach().cpu().numpy()
        return result
    except Exception as e:
        print(f"[BUDDHA/Python Error] Execution error in dynamic skill '{skill_name}': {e}")
        raise e


def list_skills() -> Dict[str, int]:
    return {name: meta["version"] for name, meta in _SKILL_REGISTRY.items()}