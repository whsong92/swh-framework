"""
swhAI BUDDHA 신경망 엔진 중앙 코어 오케스트레이터 (Central Neural Orchestrator)
- Rust(pycl) 호스트와의 CPython/PyO3 FFI 통신 및 L0 Shared Arena 제로카피 텐서 사영.
- Universal Primitives(algebra, box_embed, orthogonal) 단일 진입점 디스패치.
- torch.no_grad() 기반 실시간 국소 가소성 및 런타임 핫패칭(Hot-Patching) 엔진.
"""

import ctypes
from typing import Dict, Any, List, Optional
import numpy as np
import torch

from rbrain.memory import (
    set_shared_arena,
    get_shared_arena,
    is_bound,
    get_capacity,
    read_tensor_slice,
    write_tensor_slice
)
from rbrain.primitives.algebra import (
    concept_addition,
    concept_subtraction,
    concept_multiplication,
    concept_division,
    local_hebbian_update
)
from rbrain.primitives.box_embed import (
    eval_box_inclusion,
    create_concept_box
)
from rbrain.primitives.orthogonal import (
    eval_orthogonality,
    enforce_orthogonality,
    reflect_hyperplane
)

# 런타임 동적 마운트된 신경망 스킬 레지스트리
_DYNAMIC_SKILL_REGISTRY: Dict[str, Any] = {}


# =============================================================================
# 1. BUDDHA 엔진 진입점 및 헬스체크
# =============================================================================

def buddha(signal: str = "INIT") -> int:
    """
    BUDDHA 신경망 엔진의 정식 진입점.
    PyTorch 런타임, GPU/CPU 디바이스, Shared Memory 상태를 점검하고 성공 코드(0)를 반환.
    """
    device = "cuda" if torch.cuda.is_available() else "cpu"
    arena_status = f"BOUND ({get_capacity()} slots)" if is_bound() else "UNBOUND"
    
    msg = (
        f"[BUDDHA/Python] Neural Engine Awakened. Signal: {signal} | "
        f"PyTorch: {torch.__version__} | Device: {device} | L0 Memory: {arena_status}"
    )
    print(msg)
    return 0


def ping() -> str:
    """Rust(pycl) 호스트의 핑 호출 응답"""
    device = "cuda" if torch.cuda.is_available() else "cpu"
    if is_bound():
        return f"BUDDHA neural runtime online (device={device}, arena_slots={get_capacity()})"
    return f"BUDDHA neural runtime online (device={device}, arena=unbound)"

# =============================================================================
# 3. Universal Primitives 통합 실행 디스패처 (Rust FFI 브리지 연동)
# =============================================================================

@torch.no_grad()
def eval_primitive(primitive: str, offset_a: int, offset_b: int, dim: int = 64, **kwargs) -> Dict[str, Any]:
    """
    Rust(pycl)에서 호출하는 6대 메타 연산자 단일 실행 디스패치.
    - primitive: '+', '-', '*', '/', '⊂', '¬'
    - offset_a, offset_b: L0 Shared Arena 내 텐서 슬라이스 오프셋
    - dim: 기본 임베딩 차원 (박스 임베딩의 경우 2*dim 슬롯 사용)
    """
    if not is_bound():
        raise RuntimeError("[BUDDHA/Python Error] Shared Arena is not bound")

    out_offset = kwargs.get("out_offset", None)

    # 1. 포함 (Inclusion, ⊂): 초공간 박스 임베딩 체적 계산
    if primitive in ("⊂", "inclusion", "in"):
        temp = kwargs.get("temp", 1.0)
        return eval_box_inclusion(offset_a, offset_b, dim=dim, temp=temp)

    # 2. 반대/부정 (Opposition, ¬): 직교 사영 및 배타성 점수
    elif primitive in ("¬", "opposition", "not"):
        return eval_orthogonality(offset_a, offset_b, dim=dim)

    # 3. 더하기 (Addition, +): 개념 합성
    elif primitive in ("+", "addition", "add"):
        alpha = kwargs.get("alpha", 0.5)
        vec, norm = concept_addition(offset_a, offset_b, dim=dim, alpha=alpha, out_offset=out_offset)
        return {"primitive": "+", "norm": norm, "out_offset": out_offset}

    # 4. 빼기 (Subtraction, -): 속성 분리 및 잔차 추출
    elif primitive in ("-", "subtraction", "sub"):
        project_orthogonal = kwargs.get("project_orthogonal", True)
        vec, norm = concept_subtraction(offset_a, offset_b, dim=dim, project_orthogonal=project_orthogonal, out_offset=out_offset)
        return {"primitive": "-", "residual_norm": norm, "out_offset": out_offset}

    # 5. 곱하기 (Multiplication, *): 상호작용 강도 증폭
    elif primitive in ("*", "multiplication", "mul"):
        mode = kwargs.get("mode", "hadamard")
        vec, strength = concept_multiplication(offset_a, offset_b, dim=dim, mode=mode, out_offset=out_offset)
        return {"primitive": "*", "interaction_strength": strength, "out_offset": out_offset}

    # 6. 나누기 (Division, /): 비례 관계 및 역스케일링
    elif primitive in ("/", "division", "div"):
        vec, ratio = concept_division(offset_a, offset_b, dim=dim, out_offset=out_offset)
        return {"primitive": "/", "mean_ratio": ratio, "out_offset": out_offset}

    else:
        raise ValueError(f"[BUDDHA/Python Error] Unknown primitive operator: '{primitive}'")


# =============================================================================
# 4. 런타임 동적 핫패칭 및 가소성 제어 (Dynamic Hot-Patching)
# =============================================================================

def mount_dynamic_skill(skill_name: str, code_str: str) -> bool:
    """
    자연어 명세나 텍스트로부터 생성된 파이썬 함수/모듈 코드를 
    프로세스 중단 없이 런타임에 즉석 컴파일하여 레지스트리에 마운트.
    """
    try:
        local_scope: Dict[str, Any] = {}
        # PyTorch 및 프리미티브 환경 바인딩 제공
        global_scope = {
            "torch": torch,
            "np": np,
            "read_tensor_slice": read_tensor_slice,
            "write_tensor_slice": write_tensor_slice,
            "eval_primitive": eval_primitive
        }
        exec(code_str, global_scope, local_scope)
        
        target_fn = local_scope.get(skill_name) or next(
            (v for v in local_scope.values() if callable(v)), None
        )
        if target_fn is None:
            print(f"[BUDDHA/Python Error] No callable object found for skill '{skill_name}'")
            return False

        _DYNAMIC_SKILL_REGISTRY[skill_name] = target_fn
        print(f"[BUDDHA/Python] Dynamic skill '{skill_name}' successfully compiled and mounted.")
        return True
    except Exception as e:
        print(f"[BUDDHA/Python Error] Failed to compile dynamic skill '{skill_name}': {e}")
        return False


def call_dynamic_skill(skill_name: str, *args, **kwargs) -> Any:
    """마운트된 동적 신경망 스킬 실행"""
    fn = _DYNAMIC_SKILL_REGISTRY.get(skill_name)
    if fn is None:
        raise KeyError(f"[BUDDHA/Python Error] Skill '{skill_name}' is not mounted.")
    return fn(*args, **kwargs)