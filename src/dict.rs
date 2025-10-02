use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::submodule;
use dashmap::DashMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods};

fn none_object(py: Python<'_>) -> Py<PyAny> {
    py.None()
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "dict")?;
    module.add_class::<ConcurrentDict>()?;
    submodule::register_submodule(py, parent, &module, "syncx.dict")?;
    Ok(())
}

#[pyclass(module = "syncx.dict")]
pub struct ConcurrentDict {
    inner: Arc<DashMap<PyKey, Py<PyAny>>>,
}

struct PyKey {
    object: Py<PyAny>,
    hash: isize,
}

impl PyKey {
    fn new(raw: &Bound<'_, PyAny>) -> PyResult<Self> {
        let hash = raw.hash()?;
        Ok(Self {
            object: raw.clone().unbind(),
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
    fn new_inner() -> Arc<DashMap<PyKey, Py<PyAny>>> {
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

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py_key = Self::ensure_hashable(key)?;
        if let Some(entry) = self.inner.get(&py_key) {
            Ok(entry.value().clone_ref(py))
        } else {
            Err(PyKeyError::new_err(key.clone().unbind()))
        }
    }

    fn __setitem__(&self, key: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_key = Self::ensure_hashable(key)?;
        let value_object = value.clone().unbind();
        self.inner.insert(py_key, value_object);
        Ok(())
    }

    fn __delitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_key = Self::ensure_hashable(key)?;
        let key_object = key.clone().unbind();
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
    ) -> PyResult<Py<PyAny>> {
        let py_key = Self::ensure_hashable(key)?;
        if let Some(entry) = self.inner.get(&py_key) {
            Ok(entry.value().clone_ref(py))
        } else {
            Ok(default
                .map(Bound::unbind)
                .unwrap_or_else(|| none_object(py)))
        }
    }

    #[pyo3(signature = (key, default=None))]
    fn setdefault(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py_key = Self::ensure_hashable(key)?;
        let entry = self.inner.entry(py_key);
        Ok(match entry {
            dashmap::mapref::entry::Entry::Occupied(occupied) => occupied.get().clone_ref(py),
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                let value = default
                    .map(Bound::unbind)
                    .unwrap_or_else(|| none_object(py));
                vacant.insert(value.clone_ref(py));
                value
            }
        })
    }

    #[pyo3(signature = (key, default=None))]
    fn pop(
        &self,
        key: &Bound<'_, PyAny>,
        default: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py_key = Self::ensure_hashable(key)?;
        if let Some((_, value)) = self.inner.remove(&py_key) {
            Ok(value)
        } else if let Some(default_any) = default {
            Ok(default_any.unbind())
        } else {
            Err(PyKeyError::new_err(key.clone().unbind()))
        }
    }

    fn clear(&self) {
        self.inner.clear();
    }
}
