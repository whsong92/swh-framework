"""
swhAI BUDDHA 사칙 대수 프리미티브 (Universal Algebraic Primitives: +, -, *, /)
- Base Model의 핵심 정적 연산자로, 개념 벡터의 합성, 분리, 스케일링, 정규화 수행.
- L0 Shared Memory Arena의 NumPy 뷰(OWNDATA=False)를 제로카피로 PyTorch 텐서화하여 직접 연산.
- 전역 그래디언트 백프로파게이션 없이 torch.no_grad() 기반 국소 연산 및 가소성 갱신 적용.
"""

from typing import Tuple, Optional
import numpy as np
import torch
import torch.nn.functional as F

from rbrain.memory import get_shared_arena, read_tensor_slice, write_tensor_slice

# 기본 임베딩 차원
DEFAULT_DIM = 64
EPSILON = 1e-7


# =============================================================================
# 1. 더하기 (Addition, +): 개념 합성 및 속성 결합
# =============================================================================

@torch.no_grad()
def concept_addition(
    offset_a: int,
    offset_b: int,
    dim: int = DEFAULT_DIM,
    alpha: float = 0.5,
    out_offset: Optional[int] = None
) -> Tuple[np.ndarray, float]:
    """
    개념 A와 속성/개념 B를 합성하여 새로운 결합 벡터 생성: C = alpha * A + (1 - alpha) * B
    - 결합 후 L2 정규화를 거쳐 방향성 보존.
    - out_offset이 지정된 경우 L0 Shared Arena에 제로카피로 즉시 기록.
    """
    vec_a = read_tensor_slice(offset_a, dim)
    vec_b = read_tensor_slice(offset_b, dim)

    # 볼록 결합 (Convex Combination) 및 정규화
    combined = alpha * vec_a + (1.0 - alpha) * vec_b
    norm_val = torch.norm(combined, p=2).item()
    result_vec = F.normalize(combined, p=2, dim=0)

    if out_offset is not None:
        write_tensor_slice(out_offset, result_vec)

    print(f"[BUDDHA/Python] Primitive [+] Addition: 0x{offset_a:X} + 0x{offset_b:X} (alpha={alpha}) -> Norm: {norm_val:.4f}")
    return result_vec.cpu().numpy(), norm_val


# =============================================================================
# 2. 빼기 (Subtraction, -): 속성 분리 및 차이 추출
# =============================================================================

@torch.no_grad()
def concept_subtraction(
    offset_a: int,
    offset_b: int,
    dim: int = DEFAULT_DIM,
    project_orthogonal: bool = True,
    out_offset: Optional[int] = None
) -> Tuple[np.ndarray, float]:
    """
    개념 A에서 특정 속성/개념 B를 제거한 순수 잔차(Residual) 벡터 추출.
    - project_orthogonal=True: A에서 B 방향의 정사영 성분을 완전 제거하여 순수 직교 잔여분 산출.
      Res = A - ((A · B) / ||B||^2) * B
    """
    vec_a = read_tensor_slice(offset_a, dim)
    vec_b = read_tensor_slice(offset_b, dim)

    if project_orthogonal:
        # Gram-Schmidt 방식의 성분 추출
        b_norm_sq = torch.dot(vec_b, vec_b) + EPSILON
        proj = (torch.dot(vec_a, vec_b) / b_norm_sq) * vec_b
        residual = vec_a - proj
    else:
        residual = vec_a - vec_b

    diff_norm = torch.norm(residual, p=2).item()
    result_vec = F.normalize(residual, p=2, dim=0)

    if out_offset is not None:
        write_tensor_slice(out_offset, result_vec)

    print(f"[BUDDHA/Python] Primitive [-] Subtraction: 0x{offset_a:X} - 0x{offset_b:X} (ortho={project_orthogonal}) -> Residual Norm: {diff_norm:.4f}")
    return result_vec.cpu().numpy(), diff_norm


# =============================================================================
# 3. 곱하기 (Multiplication, *): 상호작용 강도 증폭 및 다중 선형 변환
# =============================================================================

@torch.no_grad()
def concept_multiplication(
    offset_a: int,
    offset_b: int,
    dim: int = DEFAULT_DIM,
    mode: str = "hadamard",
    out_offset: Optional[int] = None
) -> Tuple[np.ndarray, float]:
    """
    두 개념 간의 상호작용(Interaction) 강도 측정 및 비선형 융합.
    - mode="hadamard": 원소별 곱(Element-wise Product)을 통해 공통 활성화된 특성 증폭.
    - mode="outer_scalar": 코사인 유사도 기반 결합 강도 스케일링.
    """
    vec_a = read_tensor_slice(offset_a, dim)
    vec_b = read_tensor_slice(offset_b, dim)

    if mode == "hadamard":
        interacted = vec_a * vec_b
        interaction_strength = float(torch.sum(interacted).item())
        result_vec = F.normalize(interacted, p=2, dim=0)
    elif mode == "outer_scalar":
        # 내적 유사도를 강도로 삼아 A 벡터를 게이팅
        strength = float(torch.dot(F.normalize(vec_a, p=2, dim=0), F.normalize(vec_b, p=2, dim=0)).item())
        result_vec = vec_a * strength
        interaction_strength = strength
    else:
        raise ValueError(f"Unknown multiplication mode: {mode}")

    if out_offset is not None:
        write_tensor_slice(out_offset, result_vec)

    print(f"[BUDDHA/Python] Primitive [*] Multiplication: 0x{offset_a:X} * 0x{offset_b:X} (mode={mode}) -> Strength: {interaction_strength:.4f}")
    return result_vec.cpu().numpy(), interaction_strength


# =============================================================================
# 4. 나누기 (Division, /): 비례 관계, 역원 사영 및 정규화
# =============================================================================

@torch.no_grad()
def concept_division(
    offset_a: int,
    offset_b: int,
    dim: int = DEFAULT_DIM,
    out_offset: Optional[int] = None
) -> Tuple[np.ndarray, float]:
    """
    개념 A의 B에 대한 상대적 비례(Ratio) 또는 역원 사영.
    - B의 성분에 대한 정규화된 스케일 팩터 및 원소별 역스케일링 산출:
      Ratio = A / (B + sign(B) * EPSILON)
    """
    vec_a = read_tensor_slice(offset_a, dim)
    vec_b = read_tensor_slice(offset_b, dim)

    # 분모 안정화 (Zero-division 방지 부호 유지 엡실론)
    eps_b = torch.sign(vec_b) * EPSILON
    eps_b = torch.where(eps_b == 0.0, torch.full_like(eps_b, EPSILON), eps_b)
    safe_b = vec_b + eps_b

    ratio = vec_a / safe_b
    mean_ratio = float(torch.mean(ratio).item())
    result_vec = torch.tanh(ratio)  # 극단적 이상치 방지를 위한 수렴 활성화

    if out_offset is not None:
        write_tensor_slice(out_offset, result_vec)

    print(f"[BUDDHA/Python] Primitive [/] Division: 0x{offset_a:X} / 0x{offset_b:X} -> Mean Ratio: {mean_ratio:.4f}")
    return result_vec.cpu().numpy(), mean_ratio


# =============================================================================
# 5. 국소 가소성 갱신 (Local Plasticity Update - Hebbian Style)
# =============================================================================

@torch.no_grad()
def local_hebbian_update(
    offset_target: int,
    offset_context: int,
    dim: int = DEFAULT_DIM,
    learning_rate: float = 0.01
) -> float:
    """
    역전파 없는 단일 패스 국소 가소성 갱신 (Structural Imprinting).
    동시 활성화된 컨텍스트 벡터 방향으로 타깃 개념의 가중치를 미세 편향시킴:
    W_{t+1} = Normalize(W_t + lr * Context)
    """
    target = read_tensor_slice(offset_target, dim)
    context = read_tensor_slice(offset_context, dim)

    delta = learning_rate * context
    updated = F.normalize(target + delta, p=2, dim=0)

    write_tensor_slice(offset_target, updated)
    update_magnitude = float(torch.norm(delta, p=2).item())

    print(f"[BUDDHA/Python] Local Plasticity Imprinted at 0x{offset_target:X} (delta: {update_magnitude:.5f})")
    return update_magnitude