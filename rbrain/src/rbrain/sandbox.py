"""
swhAI BUDDHA 격리 실행 샌드박스 (Isolated Python Sandbox).
- Rust(pycl) 호스트 프로세스 내부에서 구동되는 인-프로세스 라이브러리 모듈.
- types.ModuleType 기반으로 전역 네임스페이스 오염 없이 외부 동적 코드를 안전하게 실행.
- AST 사전 분석을 통해 시스템 콜(os, subprocess, sys.exit 등) 차단.
- PyTorch 커널, L0 Shared Arena 뷰, 6대 대수 프리미티브 심볼 자동 주입.
"""

import ast
from typing import Dict, Any, Optional, Tuple
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
from rbrain.primitives.algebra import (
    concept_addition,
    concept_subtraction,
    concept_multiplication,
    concept_division,
    local_hebbian_update
)
from rbrain.primitives.box_embed import eval_box_inclusion, create_concept_box
from rbrain.primitives.orthogonal import eval_orthogonality, enforce_orthogonality


# -----------------------------------------------------------------------------
# 1. AST 보안 검증기 (Security Guard)
# -----------------------------------------------------------------------------

# 격리 환경에서 원천 차단할 위험 모듈 및 내장 호출 목록
BLOCKED_MODULES = {
    "os", "sys", "subprocess", "socket", "shutil", "builtins",
    "posix", "nt", "_thread", "threading", "multiprocessing"
}
BLOCKED_BUILTINS = {
    "eval", "exec", "compile", "open", "input", "__import__",
    "globals", "locals", "exit", "quit"
}


class SecurityViolationError(RuntimeError):
    """샌드박스 보안 규칙 위반 예외"""
    pass


class SandboxASTValidator(ast.NodeVisitor):
    """동적 코드 AST 트리를 탐색하여 위험 모듈 임포트 및 시스템 호출 탐지"""

    def visit_Import(self, node: ast.Import):
        for alias in node.names:
            base_mod = alias.name.split(".")[0]
            if base_mod in BLOCKED_MODULES:
                raise SecurityViolationError(
                    f"[BUDDHA/Sandbox] Forbidden import detected: '{alias.name}'"
                )
        self.generic_visit(node)

    def visit_ImportFrom(self, node: ast.ImportFrom):
        if node.module:
            base_mod = node.module.split(".")[0]
            if base_mod in BLOCKED_MODULES:
                raise SecurityViolationError(
                    f"[BUDDHA/Sandbox] Forbidden import-from detected: '{node.module}'"
                )
        self.generic_visit(node)

    def visit_Call(self, node: ast.Call):
        if isinstance(node.func, ast.Name):
            if node.func.id in BLOCKED_BUILTINS:
                raise SecurityViolationError(
                    f"[BUDDHA/Sandbox] Forbidden builtin call detected: '{node.func.id}()'"
                )
        self.generic_visit(node)


def validate_code_ast(code_str: str) -> None:
    """코드 실행 전 AST 레벨에서 보안 유효성 검증"""
    try:
        parsed_tree = ast.parse(code_str)
        validator = SandboxASTValidator()
        validator.visit(parsed_tree)
    except SyntaxError as se:
        raise SyntaxError(f"[BUDDHA/Sandbox] Code syntax error: {se}") from se


# -----------------------------------------------------------------------------
# 2. 샌드박스 컨텍스트 생성기
# -----------------------------------------------------------------------------

def create_sandbox_module(module_name: str) -> types.ModuleType:
    """
    위험 빌트인을 마스킹하고, 수치/신경망 연산에 필요한 안전한 심볼들만 화이트리스트 주입한 모듈 생성
    """
    safe_builtins = {
        "abs": abs, "all": all, "any": any, "bool": bool, "dict": dict,
        "enumerate": enumerate, "filter": filter, "float": float, "format": format,
        "int": int, "isinstance": isinstance, "issubclass": issubclass, "len": len,
        "list": list, "map": map, "max": max, "min": min, "pow": pow,
        "print": print, "range": range, "reversed": reversed, "round": round,
        "set": set, "slice": slice, "sorted": sorted, "str": str, "sum": sum,
        "tuple": tuple, "zip": zip, "None": None, "True": True, "False": False
    }

    isolated_mod = types.ModuleType(module_name)
    isolated_mod.__builtins__ = safe_builtins

    # BUDDHA 신경망 환경 및 Universal Primitives 주입
    isolated_mod.__dict__.update({
        "torch": torch,
        "nn": nn,
        "F": F,
        "np": np,
        # L0 Shared Arena 배관
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

    return isolated_mod


# -----------------------------------------------------------------------------
# 3. 샌드박스 실행 인터페이스 (라이브러리 API)
# -----------------------------------------------------------------------------

@torch.no_grad()
def eval_sandboxed_code(
    code_str: str,
    module_name: str = "transient_skill",
    target_fn_name: Optional[str] = None,
    fn_args: Optional[Tuple[Any, ...]] = None,
    fn_kwargs: Optional[Dict[str, Any]] = None
) -> Dict[str, Any]:
    """
    전역 공간 오염 없이 완전히 격리된 샌드박스 내에서 파이썬 코드를 실행.
    - code_str: 실행 또는 함수를 정의하는 파이썬 코드 문자열
    - target_fn_name: 실행 완료 후 모듈 내에서 호출할 특정 함수명 (None일 경우 코드 전체 실행)
    - fn_args / fn_kwargs: 타깃 함수에 넘길 인자
    """
    # 1. AST 보안 점검
    validate_code_ast(code_str)

    # 2. 격리된 모듈 생성
    isolated_mod = create_sandbox_module(module_name)
    args = fn_args or ()
    kwargs = fn_kwargs or {}

    try:
        # 3. 격리된 __dict__ 내에서 코드 컴파일 및 실행
        exec(code_str, isolated_mod.__dict__)

        # 4. 특정 함수 호출 요청 시
        if target_fn_name:
            if not hasattr(isolated_mod, target_fn_name):
                return {
                    "status": "error",
                    "error": f"Function '{target_fn_name}' not found in module"
                }

            callable_fn = getattr(isolated_mod, target_fn_name)
            result = callable_fn(*args, **kwargs)

            # PyTorch 텐서 결과 정규화
            if isinstance(result, torch.Tensor):
                if result.numel() == 1:
                    result = float(result.item())
                else:
                    result = result.detach().cpu().numpy()

            return {
                "status": "success",
                "result": result
            }

        return {
            "status": "success",
            "result": None
        }

    except Exception as e:
        print(f"[BUDDHA/Sandbox Error] Execution trapped: {e}")
        return {
            "status": "error",
            "error": str(e)
        }
    finally:
        # 5. 전역 네임스페이스 누출 방지를 위한 명시적 정리
        del isolated_mod