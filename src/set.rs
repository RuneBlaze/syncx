use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::submodule;
use dashmap::DashSet;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods, PyList, PyListMethods};

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "set")?;
    module.add_class::<ConcurrentSet>()?;
    submodule::register_submodule(py, parent, &module, "syncx.set")?;
    Ok(())
}

#[pyclass(module = "syncx.set")]
pub struct ConcurrentSet {
    inner: Arc<DashSet<PyKey>>,
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

    fn clone_object(&self, py: Python<'_>) -> Py<PyAny> {
        self.object.clone_ref(py)
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

impl ConcurrentSet {
    fn new_inner() -> Arc<DashSet<PyKey>> {
        Arc::new(DashSet::new())
    }

    fn ensure_hashable(value: &Bound<'_, PyAny>) -> PyResult<PyKey> {
        PyKey::new(value)
    }
}

#[pymethods]
impl ConcurrentSet {
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

    fn __contains__(&self, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        let key = Self::ensure_hashable(value)?;
        Ok(self.inner.contains(&key))
    }

    fn add(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let key = Self::ensure_hashable(value)?;
        self.inner.insert(key);
        Ok(())
    }

    fn discard(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let key = Self::ensure_hashable(value)?;
        self.inner.remove(&key);
        Ok(())
    }

    fn remove(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let key = Self::ensure_hashable(value)?;
        if self.inner.remove(&key).is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err(value.clone().unbind()))
        }
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn copy(&self) -> Self {
        let new_inner = DashSet::new();
        for entry in self.inner.iter() {
            new_inner.insert(entry.clone());
        }
        Self {
            inner: Arc::new(new_inner),
        }
    }

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let items: Vec<Py<PyAny>> = self
            .inner
            .iter()
            .map(|entry| entry.clone_object(py))
            .collect();
        Ok(PyList::new(py, items)?.unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyList>) -> PyResult<()> {
        self.inner.clear();
        for item in state.iter() {
            let key = Self::ensure_hashable(&item)?;
            self.inner.insert(key);
        }
        Ok(())
    }
}
