# rbrain/src/rbrain/dynamic_loader.py
"""
swhAI BUDDHA 런타임 동적 모듈 컴파일러 및 핫패칭 엔진
"""
from typing import Dict, Any, Optional, Callable
import types
import numpy as np
import torch

from rbrain.memory import (
    get_shared_arena,
    read_tensor_slice,
    write_tensor_slice,
    is_bound
)
from rbrain.yahweh_proxy import YahwehProxy

_MODULE_REGISTRY: Dict[str, Dict[str, Any]] = {}


def _build_sandbox_environment(module_name: str) -> types.ModuleType:
    mod = types.ModuleType(module_name)
    mod.__dict__.update({
        "__name__": module_name,
        "__doc__": f"swhAI BUDDHA Dynamic Isolated Module [{module_name}]",
        "torch": torch,
        "np": np,
        # L0 제로카피 공유 메모리 배관
        "read_tensor_slice": read_tensor_slice,
        "write_tensor_slice": write_tensor_slice,
        "is_arena_bound": is_bound,
        # Clojure 인스턴스 역호출 프록시
        "yahweh": YahwehProxy,
    })
    return mod


def register_module(module_name: str, code_str: str) -> bool:
    try:
        mod = _build_sandbox_environment(module_name)
        exec(code_str, mod.__dict__)

        entrypoint: Optional[Callable] = getattr(mod, module_name, None)
        if entrypoint is None:
            for fallback in ("run", "forward", "execute", "call"):
                if hasattr(mod, fallback) and callable(getattr(mod, fallback)):
                    entrypoint = getattr(mod, fallback)
                    break

        if entrypoint is None:
            for attr_name, attr_val in mod.__dict__.items():
                if not attr_name.startswith("_") and callable(attr_val):
                    entrypoint = attr_val
                    break

        if entrypoint is None:
            print(f"[BUDDHA/Error] No callable entrypoint found in module '{module_name}'")
            return False

        current_ver = _MODULE_REGISTRY.get(module_name, {}).get("version", 0) + 1
        _MODULE_REGISTRY[module_name] = {
            "module": mod,
            "entrypoint": entrypoint,
            "version": current_ver
        }
        print(f"[BUDDHA/Infrastructure] Module '{module_name}' (v{current_ver}) compiled and mounted.")
        return True
    except Exception as e:
        print(f"[BUDDHA/Error] Failed to compile dynamic module '{module_name}': {e}")
        return False


def unregister_module(module_name: str) -> bool:
    if module_name in _MODULE_REGISTRY:
        del _MODULE_REGISTRY[module_name]
        return True
    return False


def call_module(module_name: str, *args, **kwargs) -> Any:
    if module_name not in _MODULE_REGISTRY:
        raise RuntimeError(f"[BUDDHA/Error] Module '{module_name}' is not mounted.")

    entrypoint = _MODULE_REGISTRY[module_name]["entrypoint"]
    result = entrypoint(*args, **kwargs)
    if isinstance(result, torch.Tensor):
        if result.numel() == 1:
            return float(result.item())
        return result.detach().cpu().numpy().tolist()
    return result


def list_modules() -> Dict[str, int]:
    return {name: meta["version"] for name, meta in _MODULE_REGISTRY.items()}