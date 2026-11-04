"""
BUDDHA (Python) Cognitive Storage Interface
- pycl Rust Host의 StorageManager(Redis L1 + SurrealDB L2)를 바인딩하는 고수준 인터페이스.
- Rust FFI(pycl_storage) 연결 시 고속 C-ABI를 거치며, 미연결 시 격리된 In-Memory Mock 모드로 동작.
- Hyper-box 임베딩 파라미터 및 인과/대수적 토폴로지 관계를 L2 지식 그래프에 영구 각인(Structural Imprinting).
"""

import json
from typing import Any, Dict, List, Optional

try:
    import pycl_storage  # Rust PyO3에서 주입된 확장 모듈
except ImportError:
    pycl_storage = None


class CognitiveStorage:
    """L1 Redis 및 L2 SurrealDB를 제어하는 BUDDHA 인지 스토리지 파이프라인"""

    # FFI 부재 시 사용할 격리된 인메모리 폴백 저장소
    _mock_l1: Dict[str, str] = {}
    _mock_l2_concepts: Dict[str, Dict[str, Any]] = {}
    _mock_l2_edges: Dict[str, Dict[str, Any]] = {}

    # -------------------------------------------------------------
    # L1 Fast Working Memory (Redis 추상화)
    # -------------------------------------------------------------
    @classmethod
    def cache_set(cls, key: str, value: Any, ttl_sec: Optional[int] = None) -> bool:
        """
        L1 캐시에 임의의 상태 데이터(원시값, dict, list 등)를 JSON 형태로 저장.
        """
        serialized = json.dumps(value, ensure_ascii=False) if not isinstance(value, str) else value

        if pycl_storage is not None and hasattr(pycl_storage, "redis_set"):
            try:
                return bool(pycl_storage.redis_set(key, serialized, ttl_sec))
            except Exception as e:
                print(f"[BUDDHA/Python Error] L1 Redis SET failed: {e}")
                return False

        # In-Memory Mock Fallback
        cls._mock_l1[key] = serialized
        return True

    @classmethod
    def cache_get(cls, key: str, as_json: bool = True) -> Optional[Any]:
        """
        L1 캐시에서 키 조회. as_json=True일 경우 파이썬 객체로 역직렬화.
        """
        raw: Optional[str] = None

        if pycl_storage is not None and hasattr(pycl_storage, "redis_get"):
            try:
                raw = pycl_storage.redis_get(key)
            except Exception as e:
                print(f"[BUDDHA/Python Error] L1 Redis GET failed: {e}")
                return None
        else:
            raw = cls._mock_l1.get(key)

        if raw is None:
            return None

        if as_json:
            try:
                return json.loads(raw)
            except (json.JSONDecodeError, TypeError):
                return raw
        return raw

    @classmethod
    def cache_del(cls, key: str) -> bool:
        """L1 캐시 키 삭제"""
        if pycl_storage is not None and hasattr(pycl_storage, "redis_del"):
            try:
                return bool(pycl_storage.redis_del(key))
            except Exception as e:
                print(f"[BUDDHA/Python Error] L1 Redis DEL failed: {e}")
                return False

        return cls._mock_l1.pop(key, None) is not None

    # -------------------------------------------------------------
    # L2 Cognitive Topology Graph (SurrealDB 추상화)
    # -------------------------------------------------------------
    @classmethod
    def query(cls, surql: str) -> Any:
        """
        SurrealDB 쿼리를 실행하고 파이썬 객체(dict/list)로 파싱하여 반환.
        """
        if pycl_storage is not None and hasattr(pycl_storage, "surreal_query"):
            try:
                raw_result = pycl_storage.surreal_query(surql)
                if isinstance(raw_result, str):
                    try:
                        return json.loads(raw_result)
                    except json.JSONDecodeError:
                        return raw_result
                return raw_result
            except Exception as e:
                print(f"[BUDDHA/Python Error] L2 SurrealDB query failed: {e}")
                return {"error": str(e)}

        # In-Memory Mock Fallback
        print(f"[BUDDHA/Python Mock] Executed SurrealQL: {surql}")
        return {"status": "MOCK_OK", "query": surql}

    @classmethod
    def imprint_concept(
        cls,
        name: str,
        center: List[float],
        offset: List[float],
        attributes: Optional[Dict[str, Any]] = None,
        concept_type: str = "Concept"
    ) -> Dict[str, Any]:
        """
        Hyper-box Embedding 파라미터 및 개념 메타데이터를 SurrealDB(concept_node)에 영구 각인.
        """
        payload = {
            "id": name,
            "name": name,
            "concept_type": concept_type,
            "center": center,
            "offset": offset,
            "attributes": attributes or {}
        }

        if pycl_storage is not None and hasattr(pycl_storage, "surreal_upsert"):
            try:
                raw = pycl_storage.surreal_upsert(f"concept_node:{name}", json.dumps(payload))
                return json.loads(raw) if isinstance(raw, str) else raw
            except Exception as e:
                print(f"[BUDDHA/Python Error] Concept imprinting failed via FFI: {e}")

        # Fallback Query or Mock
        surql = f"UPSERT concept_node:{name} CONTENT {json.dumps(payload)};"
        cls._mock_l2_concepts[name] = payload
        cls.query(surql)
        print(f"[BUDDHA/Python] Concept '{name}' imprinted into L2 Cognitive Topology.")
        return payload

    @classmethod
    def record_relation(
        cls,
        from_concept: str,
        relation_primitive: str,
        to_concept: str,
        weight: float = 1.0
    ) -> Dict[str, Any]:
        """
        두 개념 간의 보편 대수 관계(⊂, ¬, 인과 전이)를 L2 토폴로지 엣지(topology_edge)로 영속화.
        """
        edge_id = f"{from_concept}_{relation_primitive}_{to_concept}"
        edge_payload = {
            "from": from_concept,
            "to": to_concept,
            "primitive": relation_primitive,
            "weight": float(weight)
        }

        surql = f"UPSERT topology_edge:{edge_id} CONTENT {json.dumps(edge_payload)};"
        cls._mock_l2_edges[edge_id] = edge_payload
        cls.query(surql)
        print(
            f"[BUDDHA/Python] Topology Edge [{from_concept}] --({relation_primitive})--> "
            f"[{to_concept}] recorded."
        )
        return edge_payload