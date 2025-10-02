use std::hash::{Hash, Hasher};
use std::sync::Arc;

use dashmap::DashMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods};

use pyo3::ffi;

type PyObject = Py<PyAny>;

fn bound_to_object(value: &Bound<'_, PyAny>) -> PyObject {
    let py = value.py();
    unsafe {
        ffi::Py_NewRef(value.as_ptr());
        Py::from_owned_ptr(py, value.as_ptr())
    }
}

fn none_object(py: Python<'_>) -> PyObject {
    py.None()
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "dict")?;
    module.add_class::<ConcurrentDict>()?;
    parent.add_submodule(&module)?;

    let sys_modules: Bound<'_, pyo3::types::PyDict> =
        py.import("sys")?.getattr("modules")?.downcast_into()?;
    sys_modules.set_item("syncx.dict", &module)?;
    Ok(())
}

#[pyclass(module = "syncx.dict")]
pub struct ConcurrentDict {
    inner: Arc<DashMap<PyKey, PyObject>>,
}

struct PyKey {
    object: PyObject,
    hash: isize,
}

impl PyKey {
    fn new(raw: &Bound<'_, PyAny>) -> PyResult<Self> {
        let hash = raw.hash()?;
        Ok(Self {
            object: bound_to_object(raw),
            hash,
        })
    }
}

impl Clone for PyKey {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            object: self.object.clone_ref(py),
            hash: self.hash,
        })
    }
}

impl PartialEq for PyKey {
    fn eq(&self, other: &Self) -> bool {
        if self.hash != other.hash {
            return false;
        }

        Python::attach(|py| {
            let lhs = self.object.bind(py);
            let rhs = other.object.bind(py);
            let rhs_ptr = rhs.as_ptr();
            let rhs_object = other.object.clone_ref(py);
            match lhs.eq(rhs_object) {
                Ok(result) => result,
                Err(_) => lhs.as_ptr() == rhs_ptr,
            }
        })
    }
}

impl Eq for PyKey {}

impl Hash for PyKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl ConcurrentDict {
    fn new_inner() -> Arc<DashMap<PyKey, PyObject>> {
        Arc::new(DashMap::new())
    }

    fn ensure_hashable(key: &Bound<'_, PyAny>) -> PyResult<PyKey> {
        PyKey::new(key)
    }
}

#[pymethods]
impl ConcurrentDict {
    #[new]
    fn new() -> Self {
        Self {
            inner: Self::new_inner(),
        }
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    fn __contains__(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py_key = Self::ensure_hashable(key)?;
        Ok(self.inner.get(&py_key).is_some())
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        let py_key = Self::ensure_hashable(key)?;
        if let Some(entry) = self.inner.get(&py_key) {
            Ok(entry.value().clone_ref(py))
        } else {
            Err(PyKeyError::new_err(bound_to_object(key)))
        }
    }

    fn __setitem__(&self, key: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_key = Self::ensure_hashable(key)?;
        let value_object = bound_to_object(value);
        self.inner.insert(py_key, value_object);
        Ok(())
    }

    fn __delitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_key = Self::ensure_hashable(key)?;
        let key_object = bound_to_object(key);
        if self.inner.remove(&py_key).is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err(key_object))
        }
    }

    #[pyo3(signature = (key, default=None))]
    fn get(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let py_key = Self::ensure_hashable(key)?;
        if let Some(entry) = self.inner.get(&py_key) {
            Ok(entry.value().clone_ref(py))
        } else {
            Ok(default
                .map(|d| bound_to_object(&d))
                .unwrap_or_else(|| none_object(py)))
        }
    }

    #[pyo3(signature = (key, default=None))]
    fn setdefault(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Option<Bound<'_, PyAny>>,
    ) -> PyResult<PyObject> {
        let py_key = Self::ensure_hashable(key)?;
        let entry = self.inner.entry(py_key);
        Ok(match entry {
            dashmap::mapref::entry::Entry::Occupied(occupied) => occupied.get().clone_ref(py),
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                let value = default
                    .map(|d| bound_to_object(&d))
                    .unwrap_or_else(|| none_object(py));
                vacant.insert(value.clone_ref(py));
                value
            }
        })
    }

    #[pyo3(signature = (key, default=None))]
    fn pop(&self, key: &Bound<'_, PyAny>, default: Option<Bound<'_, PyAny>>) -> PyResult<PyObject> {
        let py_key = Self::ensure_hashable(key)?;
        if let Some((_, value)) = self.inner.remove(&py_key) {
            Ok(value)
        } else if let Some(default_any) = default {
            Ok(bound_to_object(&default_any))
        } else {
            Err(PyKeyError::new_err(bound_to_object(key)))
        }
    }

    fn clear(&self) {
        self.inner.clear();
    }
}
