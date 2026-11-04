"""
swhAI BUDDHA 직교 사영 프리미티브 (Universal Primitive: Opposition/Negation, ¬)
- 개념 간 배타적 경계, 모순의 기하학적 분리도, 직교 사영(Orthogonal Projection) 연산.
- YAHWEH(Clojure)의 공리적 배타 판정(contradiction.clj)과 변증법적 합의를 이루는 기하학적 기저.
- Gram-Schmidt 직교화 및 Householder 초평면 반사를 통한 독립 기저 추출.
- L0 Shared Memory Arena의 제로카피 텐서를 torch.no_grad() 기반으로 직접 조작.
"""

from typing import Tuple, Dict, Any, Optional
import numpy as np
import torch
import torch.nn.functional as F

from rbrain.memory import read_tensor_slice, write_tensor_slice

DEFAULT_DIM = 64
EPSILON = 1e-7


@torch.no_grad()
def project_orthogonal(
    vec_v: torch.Tensor,
    vec_u: torch.Tensor
) -> Tuple[torch.Tensor, torch.Tensor]:
    """
    Gram-Schmidt 사영: 벡터 v를 기준 벡터 u에 대해 평행 성분(Proj)과 직교 성분(Orth)으로 분해
    - Proj_u(v) = ((v · u) / ||u||^2) * u
    - Orth_u(v) = v - Proj_u(v)
    """
    u_norm_sq = torch.dot(vec_u, vec_u) + EPSILON
    proj = (torch.dot(vec_v, vec_u) / u_norm_sq) * vec_u
    orth = vec_v - proj
    return orth, proj


@torch.no_grad()
def eval_orthogonality(
    offset_a: int,
    offset_b: int,
    dim: int = DEFAULT_DIM
) -> Dict[str, Any]:
    """
    두 개념 벡터 간의 직교성 및 배타성(Opposition, ¬) 점수 산출
    - 내적 및 코사인 유사도가 0에 수렴할수록 완벽한 직교(독립/배타적 경계) 형성.
    - opposition_score: 1.0 - |cos(A, B)| (1.0에 가까울수록 상호 배타적)
    """
    vec_a = read_tensor_slice(offset_a, dim)
    vec_b = read_tensor_slice(offset_b, dim)

    norm_a = torch.norm(vec_a, p=2) + EPSILON
    norm_b = torch.norm(vec_b, p=2) + EPSILON

    cos_sim = float((torch.dot(vec_a, vec_b) / (norm_a * norm_b)).item())
    # 절대 코사인 유사도가 0일 때 반대/직교 점수는 1.0
    opposition_score = float(np.clip(1.0 - abs(cos_sim), 0.0, 1.0))

    # 직교 임계값 판정 (절대 코사인 유사도 0.05 미만 시 완전 직교로 간주)
    is_orthogonal = bool(abs(cos_sim) < 0.05)

    result = {
        "primitive": "¬",
        "opposition_score": opposition_score,
        "cosine_similarity": cos_sim,
        "is_orthogonal": is_orthogonal,
        "dim": dim
    }

    print(
        f"[BUDDHA/Python] Primitive [¬] Opposition (0x{offset_a:X} ⊥ 0x{offset_b:X}): "
        f"Score={opposition_score:.4f}, CosSim={cos_sim:.4f}, Orthogonal={is_orthogonal}"
    )
    return result


@torch.no_grad()
def enforce_orthogonality(
    offset_target: int,
    offset_base: int,
    dim: int = DEFAULT_DIM,
    out_offset: Optional[int] = None
) -> Tuple[np.ndarray, float]:
    """
    타깃 벡터(Target)에서 기준 벡터(Base) 성분을 제거하여 완전 직교화 수행.
    - A_orth = Normalize(A - Proj_B(A))
    - out_offset이 주어지면 L0 Shared Arena에 제로카피로 즉시 덮어씀.
    """
    vec_target = read_tensor_slice(offset_target, dim)
    vec_base = read_tensor_slice(offset_base, dim)

    orth, _ = project_orthogonal(vec_target, vec_base)
    orth_norm = torch.norm(orth, p=2).item()
    result_vec = F.normalize(orth, p=2, dim=0)

    target_write = out_offset if out_offset is not None else offset_target
    write_tensor_slice(target_write, result_vec)

    print(
        f"[BUDDHA/Python] Enforced Orthogonality: Target 0x{offset_target:X} made orthogonal "
        f"to Base 0x{offset_base:X} (Residual Norm: {orth_norm:.4f}) -> Saved at 0x{target_write:X}"
    )
    return result_vec.cpu().numpy(), orth_norm


@torch.no_grad()
def reflect_hyperplane(
    offset_v: int,
    offset_normal: int,
    dim: int = DEFAULT_DIM,
    out_offset: Optional[int] = None
) -> np.ndarray:
    """
    Householder 초평면 반사 (Negation Transformation):
    법선 벡터 n에 의해 정의된 초평면을 기준으로 벡터 v를 대칭 반사하여 엄밀한 부정(¬) 상태를 구성:
    v_ref = v - 2 * (v · n / ||n||^2) * n
    """
    vec_v = read_tensor_slice(offset_v, dim)
    vec_n = read_tensor_slice(offset_normal, dim)

    n_norm_sq = torch.dot(vec_n, vec_n) + EPSILON
    reflected = vec_v - 2.0 * (torch.dot(vec_v, vec_n) / n_norm_sq) * vec_n
    result_vec = F.normalize(reflected, p=2, dim=0)

    if out_offset is not None:
        write_tensor_slice(out_offset, result_vec)

    print(f"[BUDDHA/Python] Hyperplane Reflection (¬) applied to 0x{offset_v:X} along Normal 0x{offset_normal:X}")
    return result_vec.cpu().numpy()