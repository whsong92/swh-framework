"""
swhAI BUDDHA 런타임 동적 스킬 로더 및 핫패칭 엔진 (Dynamic Hot-Patching Engine)
- 전역 런타임 오염 없이 격리된 types.ModuleType 환경 내에서 파이썬/PyTorch 코드 동적 컴파일.
- 자연어 명세로부터 컴파일된 신규 신경망 마이크로 모듈(nn.Module) 및 텐서 연산자 즉석 마운트.
- L0 Shared Memory Arena 슬라이스 및 6대 대수 프리미티브 심볼 자동 바인딩.
- torch.no_grad() 기반 0 오버헤드 국소 실행 및 핫스왑 지원.
"""

from typing import Dict, Any, Optional, Callable, Union
import types
import sys
import numpy as np
import torch
import torch.nn as nn

from rbrain.memory import (
    get_shared_arena,
    read_tensor_slice,
    write_tensor_slice,
    is_bound
)
from rbrain.primitives.algebra import (
    concept_addition,
    concept_subtraction,
    concept_multiplication,
    concept_division,
    local_hebbian_update
)
from rbrain.primitives.box_embed import eval_box_inclusion, create_concept_box
from rbrain.primitives.orthogonal import eval_orthogonality, enforce_orthogonality

# 동적으로 생성된 격리 모듈 레지스트리
# key: skill_name, value: {"module": types.ModuleType, "entrypoint": Callable, "source": str, "version": int}
_SKILL_REGISTRY: Dict[str, Dict[str, Any]] = {}


def _build_sandbox_environment(module_name: str) -> types.ModuleType:
    """
    동적 스킬이 안전하고 즉각적으로 연산할 수 있도록 
    PyTorch, NumPy, L0 메모리 제어 함수, 6대 프리미티브가 사전 주입된 격리 모듈 생성.
    """
    mod = types.ModuleType(module_name)
    mod.__dict__.update({
        "__name__": module_name,
        "__doc__": f"swhAI BUDDHA Dynamic Isolated Module [{module_name}]",
        # 텐서 및 수치 라이브러리
        "torch": torch,
        "nn": nn,
        "F": torch.nn.functional,
        "np": np,
        # L0 제로카피 공유 메모리 배관
        "get_shared_arena": get_shared_arena,
        "read_tensor_slice": read_tensor_slice,
        "write_tensor_slice": write_tensor_slice,
        "is_arena_bound": is_bound,
        # 6대 대수 프리미티브
        "concept_add": concept_addition,
        "concept_sub": concept_subtraction,
        "concept_mul": concept_multiplication,
        "concept_div": concept_division,
        "hebbian_update": local_hebbian_update,
        "box_inclusion": eval_box_inclusion,
        "create_box": create_concept_box,
        "orthogonality": eval_orthogonality,
        "enforce_ortho": enforce_orthogonality,
    })
    return mod


def register_skill(skill_name: str, code_str: str) -> bool:
    """
    전역 공간 오염 없이 독립된 types.ModuleType 안에서 코드를 컴파일하고 보관.
    함수뿐만 아니라 PyTorch nn.Module 인스턴스/클래스도 진입점으로 자동 인식.
    """
    try:
        mod = _build_sandbox_environment(skill_name)
        
        # 격리된 모듈 딕셔너리 안에서만 컴파일 및 적재
        exec(code_str, mod.__dict__)

        # 진입점(Callable) 탐색 우선순위:
        # 1. skill_name과 동일한 이름의 객체
        # 2. 'forward', 'run', 'execute' 명칭의 함수/모듈
        # 3. 모듈 내 정의된 첫 번째 호출 가능 객체(함수 또는 nn.Module)
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

        # nn.Module 클래스인 경우 기본 인스턴스화 수행
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
    """
    등록된 동적 스킬 모듈에서 진입점을 찾아 실행.
    - torch.no_grad() 환경에서 무오버헤드로 안전하게 실행.
    - 다형적 인자(*args, **kwargs) 수용 및 기존 FFI(float 인자) 완벽 호환.
    """
    if skill_name not in _SKILL_REGISTRY:
        raise RuntimeError(f"[BUDDHA/Python Error] Skill '{skill_name}' is not registered.")

    entrypoint = _SKILL_REGISTRY[skill_name]["entrypoint"]

    try:
        result = entrypoint(*args, **kwargs)
        
        # PyTorch Tensor 반환 시 상황에 따라 안전하게 변환
        if isinstance(result, torch.Tensor):
            if result.numel() == 1:
                return float(result.item())
            return result.detach().cpu().numpy()
            
        return result

    except Exception as e:
        print(f"[BUDDHA/Python Error] Execution error in dynamic skill '{skill_name}': {e}")
        raise e


def call_numeric_skill(skill_name: str, arg: float) -> float:
    """기존 FFI 인터페이스 호환용 (단일 float 입출력)"""
    res = call_skill(skill_name, arg)
    return float(res)


def list_skills() -> Dict[str, int]:
    """현재 마운트된 모든 동적 스킬 및 버전 조회"""
    return {name: meta["version"] for name, meta in _SKILL_REGISTRY.items()}