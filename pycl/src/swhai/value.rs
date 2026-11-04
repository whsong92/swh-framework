// pycl/src/swhai/value.rs
use anyhow::Result;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyString};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// [swhAI] 3계층 하이브리드 엔진 간 데이터 교환 통합 열거형
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BrainValue {
    Nil,
    Boolean(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Json(JsonValue),
    TensorSlice { offset: usize, length: usize },
    TensorPtr { ptr: usize, size: usize },
}

impl BrainValue {
    #[inline]
    pub fn is_nil(&self) -> bool {
        matches!(self, BrainValue::Nil)
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            BrainValue::Float(f) => Some(*f),
            BrainValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            BrainValue::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn to_sexp(&self) -> String {
        match self {
            BrainValue::Nil => "nil".to_string(),
            BrainValue::Boolean(b) => if *b { "true".to_string() } else { "false".to_string() },
            BrainValue::Int(i) => i.to_string(),
            BrainValue::Float(f) => format!("{:.6}", f),
            BrainValue::Text(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            BrainValue::Bytes(b) => format!("\"#bytes[{}]\"", b.len()),
            BrainValue::Json(j) => serde_json::to_string(j).unwrap_or_else(|_| "nil".to_string()),
            BrainValue::TensorSlice { offset, length } => {
                format!("(:tensor-slice {} {})", offset, length)
            }
            BrainValue::TensorPtr { ptr, size } => {
                format!("(:raw-ptr 0x{:x} {})", ptr, size)
            }
        }
    }

    pub fn from_py_bound<'py>(obj: &Bound<'py, PyAny>) -> Result<Self> {
        if obj.is_none() {
            return Ok(BrainValue::Nil);
        }

        if let Ok(b) = obj.downcast::<PyBool>() {
            return Ok(BrainValue::Boolean(b.is_true()));
        }

        if let Ok(i) = obj.downcast::<PyInt>() {
            let val: i64 = i.extract()?;
            return Ok(BrainValue::Int(val));
        }

        if let Ok(f) = obj.downcast::<PyFloat>() {
            let val: f64 = f.extract()?;
            return Ok(BrainValue::Float(val));
        }

        if let Ok(s) = obj.downcast::<PyString>() {
            let val: String = s.extract()?;
            return Ok(BrainValue::Text(val));
        }

        if let Ok(bytes) = obj.downcast::<PyBytes>() {
            return Ok(BrainValue::Bytes(bytes.as_bytes().to_vec()));
        }

        if let Ok(dict) = obj.downcast::<PyDict>() {
            let py = obj.py();
            let json_module = py.import("json")?;
            let json_str: String = json_module.call_method1("dumps", (dict,))?.extract()?;
            let parsed: JsonValue = serde_json::from_str(&json_str)?;
            return Ok(BrainValue::Json(parsed));
        }

        Ok(BrainValue::Text(obj.to_string()))
    }

    pub fn to_py_object<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            BrainValue::Nil => Ok(py.None().into_bound(py)),
            // PyBool은 대여 참조 이슈가 있으므로 PyBool::new를 사용
            BrainValue::Boolean(b) => Ok(PyBool::new(py, *b).to_owned().into_any()),
            // i64, f64는 Copy 타입이므로 into_pyobject가 직접 소유권을 넘겨줍니다.
            BrainValue::Int(i) => Ok((*i).into_pyobject(py)?.into_any()),
            BrainValue::Float(f) => Ok((*f).into_pyobject(py)?.into_any()),
            BrainValue::Text(s) => Ok(PyString::new(py, s.as_str()).into_any()),
            BrainValue::Bytes(b) => Ok(PyBytes::new(py, b).into_any()),
            BrainValue::Json(j) => {
                let json_str = serde_json::to_string(j)
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
                let json_module = py.import("json")?;
                let dict = json_module.call_method1("loads", (json_str,))?;
                Ok(dict)
            }
            BrainValue::TensorSlice { offset, length } => {
                let dict = PyDict::new(py);
                dict.set_item("offset", offset)?;
                dict.set_item("length", length)?;
                Ok(dict.into_any())
            }
            BrainValue::TensorPtr { ptr, size } => {
                let ctypes = py.import("ctypes")?;
                let c_void_p = ctypes.getattr("c_void_p")?;
                let cast_fn = ctypes.getattr("cast")?;
                let c_addr = cast_fn.call1((*ptr, c_void_p))?;

                // PyInt::new 대신 (*size as i64).into_pyobject(py)?.into_any() 사용
                let size_obj = (*size as i64).into_pyobject(py)?.into_any();
                let tuple = pyo3::types::PyTuple::new(py, &[c_addr, size_obj])?;
                Ok(tuple.into_any())
            }
        }
    }
}