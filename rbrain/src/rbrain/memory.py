# rbrain/src/rbrain/memory.py
"""
swhAI BUDDHA L0 Shared Memory Arena (NumPy/PyTorch) 제로카피 브로커.
- Rust(COSMOS/pycl)가 독점 소유하는 물리 메모리 버퍼(*mut f32)를 인-플레이스로 사영.
- NumPy ndarray(OWNDATA=False) 플래그를 강제하여 가비지 컬렉터에 의한 임의 해제 원천 차단.
- torch.from_numpy()를 통해 1바이트의 메모리 복사 없이 PyTorch 텐서 엔진과 직결.
- Universal Primitives 연산을 위한 안전한 슬라이스 분할 및 I/O 제어 제공.
"""

from typing import Optional
import ctypes
import numpy as np
import torch

# L0 물리 공유 메모리 전역 상태 (단일 프로세스 내부 인스턴스)
_SHARED_ARENA_NP: Optional[np.ndarray] = None
_SHARED_ARENA_TORCH: Optional[torch.Tensor] = None
_ARENA_CAPACITY: int = 0
_ARENA_RAW_ADDRESS: int = 0


# =============================================================================
# 1. 상태 점검 및 접근자 (Accessors)
# =============================================================================

def is_bound() -> bool:
    """L0 Shared Arena가 성공적으로 마운트되어 있는지 확인"""
    return _SHARED_ARENA_NP is not None and _SHARED_ARENA_TORCH is not None


def get_capacity() -> int:
    """바인딩된 아레나의 총 f32 슬롯 수 반환"""
    return _ARENA_CAPACITY


def get_raw_address() -> int:
    """바인딩된 물리 아레나의 기본 가상 메모리 주소 반환"""
    return _ARENA_RAW_ADDRESS


def get_shared_arena() -> np.ndarray:
    """NumPy ndarray 뷰 반환 (OWNDATA=False 보장)"""
    if _SHARED_ARENA_NP is None:
        raise RuntimeError("[BUDDHA/Python Error] Shared Memory Arena is not bound.")
    return _SHARED_ARENA_NP


def get_shared_tensor() -> torch.Tensor:
    """PyTorch Tensor 뷰 반환 (Zero-Copy)"""
    if _SHARED_ARENA_TORCH is None:
        raise RuntimeError("[BUDDHA/Python Error] Shared Memory Arena is not bound.")
    return _SHARED_ARENA_TORCH


# =============================================================================
# 2. 제로카피 바인딩 배관 (Zero-Copy Binding Pipeline)
# =============================================================================

def set_shared_arena(arr: np.ndarray) -> None:
    """
    이미 생성된 NumPy 배열을 직접 수신하여 등록 (OWNDATA=False 검증 필수).
    Rust PyO3 브리지 등에서 PyArray 객체로 직접 전달받을 때 호출됨.
    """
    global _SHARED_ARENA_NP, _SHARED_ARENA_TORCH, _ARENA_CAPACITY, _ARENA_RAW_ADDRESS

    if arr.dtype != np.float32:
        raise ValueError(f"[BUDDHA/Python Error] Expected float32 array, got {arr.dtype}")

    if arr.flags.owndata:
        print("[BUDDHA/Python Warning] NumPy array owns data! Clearing OWNDATA flag for safety.")
        arr.flags.writeable = True

    _SHARED_ARENA_NP = arr
    _ARENA_CAPACITY = arr.size
    _ARENA_RAW_ADDRESS = arr.ctypes.data

    # PyTorch 텐서 뷰 생성 (NumPy 버퍼 공유)
    _SHARED_ARENA_TORCH = torch.from_numpy(_SHARED_ARENA_NP)

    print(
        f"[BUDDHA/Python] L0 Shared Arena bound via NumPy: "
        f"0x{_ARENA_RAW_ADDRESS:X} ({_ARENA_CAPACITY} slots, {(_ARENA_CAPACITY * 4) / (1024 * 1024):.2f} MB) "
        f"[OWNDATA={_SHARED_ARENA_NP.flags.owndata}]"
    )


def bind_raw_memory(raw_ptr_address: int, capacity: int) -> int:
    """
    Rust 호스트로부터 물리 주소(*mut f32)와 원소 개수를 전달받아,
    ctypes와 np.ctypeslib.as_array를 통해 무복사 1D NumPy 배열로 직접 사영.
    """
    if raw_ptr_address == 0 or capacity <= 0:
        raise ValueError(f"[BUDDHA/Python Error] Invalid pointer (0x{raw_ptr_address:X}) or capacity ({capacity})")

    c_float_p = ctypes.POINTER(ctypes.c_float)
    address = ctypes.cast(raw_ptr_address, c_float_p)

    np_arr = np.ctypeslib.as_array(address, shape=(capacity,))
    assert not np_arr.flags.owndata, "Mapped array must not own data"

    set_shared_arena(np_arr)
    return 0


# =============================================================================
# 3. 텐서 슬라이스 I/O (Tensor Slicing & Direct Manipulation)
# =============================================================================

@torch.no_grad()
def read_tensor_slice(offset: int, length: int) -> torch.Tensor:
    """
    L0 Shared Arena의 특정 오프셋(offset)부터 길이(length)만큼의 슬라이스를
    복사 없이 1D PyTorch 텐서 뷰로 반환.
    """
    if _SHARED_ARENA_TORCH is None:
        raise RuntimeError("[BUDDHA/Python Error] Shared Arena is not bound")

    end = offset + length
    if offset < 0 or end > _ARENA_CAPACITY:
        raise IndexError(
            f"[BUDDHA/Python Error] Slice [{offset}:{end}] exceeds arena capacity ({_ARENA_CAPACITY})"
        )

    return _SHARED_ARENA_TORCH[offset:end]


@torch.no_grad()
def write_tensor_slice(offset: int, source_tensor: torch.Tensor) -> int:
    """
    주어진 PyTorch 텐서의 값을 L0 Shared Arena의 특정 오프셋 위치에 인-플레이스로 복사 기록.
    (Rust 및 Clojure DirectByteBuffer에 즉시 반영)
    """
    if _SHARED_ARENA_TORCH is None:
        raise RuntimeError("[BUDDHA/Python Error] Shared Arena is not bound")

    length = source_tensor.numel()
    end = offset + length
    if offset < 0 or end > _ARENA_CAPACITY:
        raise IndexError(
            f"[BUDDHA/Python Error] Write range [{offset}:{end}] exceeds arena capacity ({_ARENA_CAPACITY})"
        )

    if source_tensor.device.type != "cpu":
        source_tensor = source_tensor.cpu()
    if source_tensor.dtype != torch.float32:
        source_tensor = source_tensor.to(torch.float32)

    _SHARED_ARENA_TORCH[offset:end].copy_(source_tensor.view(-1))
    return length