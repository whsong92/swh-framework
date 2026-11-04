"""
swhAI BUDDHA 박스 임베딩 프리미티브 (Universal Primitive: Inclusion, ⊂)
- 개념을 초공간 박스(Hyper-rectangle, [min_point, max_point])로 모델링.
- 실체(Substance)와 속성(Attribute), 상위-하위 개념 간의 체적 포함도(Soft Inclusion Volume) 연산.
- YAHWEH(Clojure)의 이산 타입 계층 검증과 변증법적 합의(Dialectic Agreement)를 이루는 기하학적 기저.
- L0 Shared Memory Arena의 메모리를 제로카피로 PyTorch 텐서화하여 직접 조작.
"""

from typing import Tuple, Dict, Any, Optional
import numpy as np
import torch
import torch.nn.functional as F

from rbrain.memory import read_tensor_slice, write_tensor_slice

DEFAULT_DIM = 64
EPSILON = 1e-7


class BoxTensor:
    """
    초공간 박스 래퍼 클래스
    - shape: (dim,)인 min_point와 max_point의 쌍 (총 2 * dim 슬롯 점유)
    """
    def __init__(self, min_point: torch.Tensor, max_point: torch.Tensor):
        # min_point <= max_point 보장
        self.min = torch.min(min_point, max_point)
        self.max = torch.max(min_point, max_point)
        self.dim = self.min.shape[0]

    @classmethod
    def from_raw_tensor(cls, raw: torch.Tensor, dim: int = DEFAULT_DIM) -> "BoxTensor":
        """2*dim 크기의 연속 텐서로부터 [min, max] 분할 생성"""
        min_p = raw[:dim]
        max_p = raw[dim:2*dim]
        return cls(min_p, max_p)

    def to_raw_tensor(self) -> torch.Tensor:
        """L0 Arena 기록용 2*dim 단일 텐서로 병합"""
        return torch.cat([self.min, self.max], dim=0)

    def volume(self, soft: bool = True, temp: float = 1.0) -> torch.Tensor:
        """
        초공간 박스 부피(체적) 계산
        - soft=True: Softplus를 사용하여 경계면에서의 미분 가능성 및 연속성 보장
        """
        diff = self.max - self.min
        if soft:
            # 부드러운 양수 체적 수렴: prod(softplus(diff / temp) * temp + eps)
            side_lengths = F.softplus(diff / temp) * temp + EPSILON
            # 로그 체적으로 계산 후 exp 변환하여 오버플로 방지
            log_vol = torch.sum(torch.log(side_lengths))
            return torch.exp(log_vol)
        else:
            side_lengths = F.relu(diff) + EPSILON
            return torch.prod(side_lengths)


@torch.no_grad()
def intersection(box_a: BoxTensor, box_b: BoxTensor) -> Optional[BoxTensor]:
    """두 초공간 박스의 교집합 박스 생성: [max(min_a, min_b), min(max_a, max_b)]"""
    inter_min = torch.max(box_a.min, box_b.min)
    inter_max = torch.min(box_a.max, box_b.max)

    # 교집합 존재 여부 검사 (단 1차원이라도 min > max 면 공집합)
    if torch.any(inter_min > inter_max):
        return None
    return BoxTensor(inter_min, inter_max)


@torch.no_grad()
def eval_box_inclusion(
    offset_a: int,
    offset_b: int,
    dim: int = DEFAULT_DIM,
    temp: float = 1.0
) -> Dict[str, Any]:
    """
    포함(Inclusion, A ⊂ B) 기하학적 확률/체적 점수 산출
    - offset_a: 개념 A (2*dim 슬롯: min, max)
    - offset_b: 상위 개념 B (2*dim 슬롯: min, max)
    - P(A ⊂ B) = Vol(A ∩ B) / Vol(A)
    """
    total_slots = 2 * dim
    raw_a = read_tensor_slice(offset_a, total_slots)
    raw_b = read_tensor_slice(offset_b, total_slots)

    box_a = BoxTensor.from_raw_tensor(raw_a, dim)
    box_b = BoxTensor.from_raw_tensor(raw_b, dim)

    vol_a = box_a.volume(soft=True, temp=temp)
    vol_b = box_b.volume(soft=True, temp=temp)

    # 교집합 박스 계산
    inter_min = torch.max(box_a.min, box_b.min)
    inter_max = torch.min(box_a.max, box_b.max)
    inter_box = BoxTensor(inter_min, inter_max)
    vol_inter = inter_box.volume(soft=True, temp=temp)

    # 포함도 점수 (0.0 ~ 1.0 클램핑)
    inclusion_score = float(torch.clamp(vol_inter / (vol_a + EPSILON), 0.0, 1.0).item())

    # B가 A를 완전히 감싸고 있는지 엄밀한 이산 포함 여부 검사
    is_strictly_contained = bool(torch.all(box_b.min <= box_a.min) and torch.all(box_b.max >= box_a.max))

    result = {
        "primitive": "⊂",
        "inclusion_score": inclusion_score,
        "is_strictly_contained": is_strictly_contained,
        "vol_a": float(vol_a.item()),
        "vol_b": float(vol_b.item()),
        "vol_inter": float(vol_inter.item()),
        "dim": dim
    }

    print(
        f"[BUDDHA/Python] Primitive [⊂] Box Inclusion (0x{offset_a:X} ⊂ 0x{offset_b:X}): "
        f"Score={inclusion_score:.4f}, Strict={is_strictly_contained}"
    )
    return result


@torch.no_grad()
def create_concept_box(
    offset_center: int,
    radius: float = 0.5,
    dim: int = DEFAULT_DIM,
    out_offset: Optional[int] = None
) -> Tuple[np.ndarray, np.ndarray]:
    """
    단일 중심 임베딩 벡터로부터 대칭 초공간 박스 생성: [Center - Radius, Center + Radius]
    - out_offset에 2*dim 길이로 즉시 제로카피 기록
    """
    center = read_tensor_slice(offset_center, dim)
    r = torch.full_like(center, abs(radius))

    box = BoxTensor(center - r, center + r)
    raw_combined = box.to_raw_tensor()

    if out_offset is not None:
        write_tensor_slice(out_offset, raw_combined)

    print(f"[BUDDHA/Python] Created Box Embedding around 0x{offset_center:X} with radius={radius}")
    return box.min.cpu().numpy(), box.max.cpu().numpy()