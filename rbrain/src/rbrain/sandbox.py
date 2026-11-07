"""
swhAI BUDDHA 격리 실행 샌드박스 (Isolated Python Sandbox).
- Rust(pycl) 호스트 프로세스 내부에서 구동되는 인-프로세스 라이브러리 모듈.
- AST 사전 분석을 통해 시스템 콜(os, subprocess, sys.exit 등) 차단.
- 정적 프리미티브 의존성을 배제하고, PyTorch 텐서 엔진 및 L0 Shared Arena 뷰만 안전하게 제공.
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

# -----------------------------------------------------------------------------
# 1. AST 보안 검증기 (Security Guard)
# -----------------------------------------------------------------------------

BLOCKED_MODULES = {
    "os", "sys", "subprocess", "socket", "shutil", "builtins",
    "posix", "nt", "_thread", "threading", "multiprocessing"
}
BLOCKED_BUILTINS = {
    "eval", "exec", "compile", "open", "input", "__import__",
    "globals", "locals", "exit", "quit"
}


class SecurityViolationError(RuntimeError):
    pass


class SandboxASTValidator(ast.NodeVisitor):
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

    # L0 제로카피 배관 및 순수 텐서 라이브러리만 주입
    isolated_mod.__dict__.update({
        "torch": torch,
        "nn": nn,
        "F": F,
        "np": np,
        "read_tensor_slice": read_tensor_slice,
        "write_tensor_slice": write_tensor_slice,
        "is_arena_bound": is_bound,
    })

    return isolated_mod


# -----------------------------------------------------------------------------
# 3. 샌드박스 실행 인터페이스
# -----------------------------------------------------------------------------

@torch.no_grad()
def eval_sandboxed_code(
    code_str: str,
    module_name: str = "transient_skill",
    target_fn_name: Optional[str] = None,
    fn_args: Optional[Tuple[Any, ...]] = None,
    fn_kwargs: Optional[Dict[str, Any]] = None
) -> Dict[str, Any]:
    validate_code_ast(code_str)
    isolated_mod = create_sandbox_module(module_name)
    args = fn_args or ()
    kwargs = fn_kwargs or {}

    try:
        exec(code_str, isolated_mod.__dict__)

        if target_fn_name:
            if not hasattr(isolated_mod, target_fn_name):
                return {
                    "status": "error",
                    "error": f"Function '{target_fn_name}' not found in module"
                }

            callable_fn = getattr(isolated_mod, target_fn_name)
            result = callable_fn(*args, **kwargs)

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
        del isolated_mod