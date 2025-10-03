use std::collections::hash_map::RandomState;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use crate::submodule;
use dashmap::iter::Iter as DashMapIter;
use dashmap::iter_set::Iter as DashSetIter;
use dashmap::mapref::entry::Entry;
use dashmap::{DashMap, DashSet};
use flume::{Receiver, RecvTimeoutError, Sender, TryRecvError, TrySendError};
use flume::{SendError, SendTimeoutError};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyAnyMethods, PyDict, PyDictMethods, PyList, PyListMethods};
use pyo3::Python;

pyo3::create_exception!(collections_module, Empty, pyo3::exceptions::PyException);
pyo3::create_exception!(collections_module, Full, pyo3::exceptions::PyException);

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "collections")?;
    module.add_class::<Queue>()?;
    module.add("Empty", py.get_type::<Empty>())?;
    module.add("Full", py.get_type::<Full>())?;
    module.add_class::<ConcurrentDict>()?;
    module.add_class::<ConcurrentSet>()?;
    submodule::register_submodule(py, parent, &module, "syncx.collections")?;
    Ok(())
}

#[pyclass(module = "syncx.collections")]
pub struct Queue {
    sender: Sender<Py<PyAny>>,
    receiver: Receiver<Py<PyAny>>,
    maxsize: Option<usize>,
}

#[pymethods]
impl Queue {
    #[new]
    #[pyo3(signature = (maxsize=0))]
    fn new(maxsize: usize) -> Self {
        let (sender, receiver) = if maxsize == 0 {
            flume::unbounded()
        } else {
            flume::bounded(maxsize)
        };

        Self {
            sender,
            receiver,
            maxsize: if maxsize == 0 { None } else { Some(maxsize) },
        }
    }

    #[getter]
    fn maxsize(&self) -> usize {
        self.maxsize.unwrap_or(0)
    }

    fn qsize(&self) -> usize {
        self.receiver.len()
    }

    fn __len__(&self) -> usize {
        self.qsize()
    }

    fn empty(&self) -> bool {
        self.receiver.is_empty()
    }

    fn full(&self) -> bool {
        match self.maxsize {
            Some(_) => self.sender.is_full(),
            None => false,
        }
    }

    #[pyo3(signature = (item, block=true, timeout=None))]
    fn put(
        &self,
        py: Python<'_>,
        item: &Bound<'_, PyAny>,
        block: bool,
        timeout: Option<f64>,
    ) -> PyResult<()> {
        let object = item.clone().unbind();

        if !block {
            return match self.sender.try_send(object) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => Err(Full::new_err("queue is full")),
                Err(TrySendError::Disconnected(_)) => {
                    Err(PyRuntimeError::new_err("queue disconnected"))
                }
            };
        }

        let duration = match timeout {
            Some(value) => Some(timeout_to_duration(value)?),
            None => None,
        };

        match duration {
            Some(duration) => match py.detach(|| self.sender.send_timeout(object, duration)) {
                Ok(()) => Ok(()),
                Err(SendTimeoutError::Timeout(_)) => Err(Full::new_err("queue is full")),
                Err(SendTimeoutError::Disconnected(_)) => {
                    Err(PyRuntimeError::new_err("queue disconnected"))
                }
            },
            None => match py.detach(|| self.sender.send(object)) {
                Ok(()) => Ok(()),
                Err(SendError(_)) => Err(PyRuntimeError::new_err("queue disconnected")),
            },
        }
    }

    fn put_nowait(&self, py: Python<'_>, item: &Bound<'_, PyAny>) -> PyResult<()> {
        self.put(py, item, false, None)
    }

    #[pyo3(signature = (block=true, timeout=None))]
    fn get(&self, py: Python<'_>, block: bool, timeout: Option<f64>) -> PyResult<Py<PyAny>> {
        if !block {
            return match self.receiver.try_recv() {
                Ok(value) => Ok(value),
                Err(TryRecvError::Empty) => Err(Empty::new_err("queue is empty")),
                Err(TryRecvError::Disconnected) => {
                    Err(PyRuntimeError::new_err("queue disconnected"))
                }
            };
        }

        let duration = match timeout {
            Some(value) => Some(timeout_to_duration(value)?),
            None => None,
        };

        match duration {
            Some(duration) => match py.detach(|| self.receiver.recv_timeout(duration)) {
                Ok(value) => Ok(value),
                Err(RecvTimeoutError::Timeout) => Err(Empty::new_err("queue is empty")),
                Err(RecvTimeoutError::Disconnected) => {
                    Err(PyRuntimeError::new_err("queue disconnected"))
                }
            },
            None => match py.detach(|| self.receiver.recv()) {
                Ok(value) => Ok(value),
                Err(flume::RecvError::Disconnected) => {
                    Err(PyRuntimeError::new_err("queue disconnected"))
                }
            },
        }
    }

    fn get_nowait(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.get(py, false, None)
    }
}

fn timeout_to_duration(timeout: f64) -> PyResult<Duration> {
    if !timeout.is_finite() {
        return Err(PyValueError::new_err("timeout must be finite"));
    }
    if timeout < 0.0 {
        return Err(PyValueError::new_err("timeout must be >= 0"));
    }

    if timeout >= (u64::MAX as f64) {
        return Ok(Duration::new(u64::MAX, 999_999_999));
    }

    let secs = timeout.trunc();
    let nanos = ((timeout - secs) * 1_000_000_000.0).round();
    let nanos = nanos.clamp(0.0, 999_999_999.0);

    Ok(Duration::new(secs as u64, nanos as u32))
}

#[pyclass(module = "syncx.collections")]
pub struct ConcurrentDict {
    inner: Arc<DashMap<DictKey, Py<PyAny>>>,
}

struct DictKey {
    object: Py<PyAny>,
    hash: isize,
}

impl DictKey {
    fn new(raw: &Bound<'_, PyAny>) -> PyResult<Self> {
        let hash = raw.hash()?;
        Ok(Self {
            object: raw.clone().unbind(),
            hash,
        })
    }
}

impl Clone for DictKey {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            object: self.object.clone_ref(py),
            hash: self.hash,
        })
    }
}

impl PartialEq for DictKey {
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

impl Eq for DictKey {}

impl Hash for DictKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

#[pyclass(module = "syncx.collections", unsendable)]
struct ConcurrentDictIter {
    _owner: Arc<DashMap<DictKey, Py<PyAny>>>,
    iter: DashMapIter<'static, DictKey, Py<PyAny>>,
}

impl ConcurrentDictIter {
    fn new(owner: Arc<DashMap<DictKey, Py<PyAny>>>) -> Self {
        let iter = unsafe { dict_iter_from_arc(&owner) };
        Self {
            _owner: owner,
            iter,
        }
    }
}

#[pymethods]
impl ConcurrentDictIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<Py<PyAny>> {
        let py = slf.py();
        slf.iter
            .next()
            .map(|entry| entry.key().object.clone_ref(py))
    }
}

unsafe fn dict_iter_from_arc(
    owner: &Arc<DashMap<DictKey, Py<PyAny>>>,
) -> DashMapIter<'static, DictKey, Py<PyAny>> {
    let map_ref: &DashMap<DictKey, Py<PyAny>> = owner.as_ref();
    let iter = map_ref.iter();
    // SAFETY: we tie the iterator lifetime to the stored Arc to keep the map alive
    // for at least as long as the iterator exists.
    std::mem::transmute::<
        DashMapIter<'_, DictKey, Py<PyAny>>,
        DashMapIter<'static, DictKey, Py<PyAny>>,
    >(iter)
}

impl ConcurrentDict {
    fn new_inner() -> Arc<DashMap<DictKey, Py<PyAny>>> {
        Arc::new(DashMap::new())
    }

    fn ensure_hashable(key: &Bound<'_, PyAny>) -> PyResult<DictKey> {
        DictKey::new(key)
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

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<ConcurrentDictIter>> {
        Py::new(py, ConcurrentDictIter::new(self.inner.clone()))
    }

    fn __contains__(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        let dict_key = Self::ensure_hashable(key)?;
        Ok(self.inner.get(&dict_key).is_some())
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let dict_key = Self::ensure_hashable(key)?;
        if let Some(entry) = self.inner.get(&dict_key) {
            Ok(entry.value().clone_ref(py))
        } else {
            Err(PyKeyError::new_err(key.clone().unbind()))
        }
    }

    fn __setitem__(&self, key: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let dict_key = Self::ensure_hashable(key)?;
        let value_object = value.clone().unbind();
        self.inner.insert(dict_key, value_object);
        Ok(())
    }

    fn __delitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let dict_key = Self::ensure_hashable(key)?;
        let key_object = key.clone().unbind();
        if self.inner.remove(&dict_key).is_some() {
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
        let dict_key = Self::ensure_hashable(key)?;
        if let Some(entry) = self.inner.get(&dict_key) {
            Ok(entry.value().clone_ref(py))
        } else {
            Ok(default.map(Bound::unbind).unwrap_or_else(|| py.None()))
        }
    }

    #[pyo3(signature = (key, default=None))]
    fn setdefault(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Option<Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let dict_key = Self::ensure_hashable(key)?;
        let entry = self.inner.entry(dict_key);
        Ok(match entry {
            Entry::Occupied(occupied) => occupied.get().clone_ref(py),
            Entry::Vacant(vacant) => {
                let value = default.map(Bound::unbind).unwrap_or_else(|| py.None());
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
        let dict_key = Self::ensure_hashable(key)?;
        if let Some((_, value)) = self.inner.remove(&dict_key) {
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

    fn __getstate__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let snapshot = PyDict::new(py);
        for entry in self.inner.iter() {
            let key = entry.key().object.clone_ref(py);
            let value = entry.value().clone_ref(py);
            snapshot.set_item(key, value)?;
        }
        Ok(snapshot.unbind())
    }

    fn __setstate__(&mut self, state: &Bound<'_, PyDict>) -> PyResult<()> {
        self.inner.clear();
        for (key, value) in state.iter() {
            let dict_key = Self::ensure_hashable(&key)?;
            let value_object = value.clone().unbind();
            self.inner.insert(dict_key, value_object);
        }
        Ok(())
    }
}

#[pyclass(module = "syncx.collections")]
pub struct ConcurrentSet {
    inner: Arc<DashSet<SetKey>>,
}

struct SetKey {
    object: Py<PyAny>,
    hash: isize,
}

impl SetKey {
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

impl Clone for SetKey {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            object: self.object.clone_ref(py),
            hash: self.hash,
        })
    }
}

impl PartialEq for SetKey {
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

impl Eq for SetKey {}

impl Hash for SetKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

#[pyclass(module = "syncx.collections", unsendable)]
struct ConcurrentSetIter {
    _owner: Arc<DashSet<SetKey>>,
    iter: DashSetIter<'static, SetKey, RandomState, DashMap<SetKey, (), RandomState>>,
}

impl ConcurrentSetIter {
    fn new(owner: Arc<DashSet<SetKey>>) -> Self {
        let iter = unsafe { set_iter_from_arc(&owner) };
        Self {
            _owner: owner,
            iter,
        }
    }
}

#[pymethods]
impl ConcurrentSetIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<Py<PyAny>> {
        let py = slf.py();
        slf.iter.next().map(|entry| entry.key().clone_object(py))
    }
}

unsafe fn set_iter_from_arc(
    owner: &Arc<DashSet<SetKey>>,
) -> DashSetIter<'static, SetKey, RandomState, DashMap<SetKey, (), RandomState>> {
    let set_ref: &DashSet<SetKey> = owner.as_ref();
    let iter = set_ref.iter();
    // SAFETY: the iterator borrows from the underlying DashSet. The stored Arc keeps
    // the set alive for the full lifetime of the iterator, so extending the lifetime
    // to 'static is sound.
    std::mem::transmute::<
        DashSetIter<'_, SetKey, RandomState, DashMap<SetKey, (), RandomState>>,
        DashSetIter<'static, SetKey, RandomState, DashMap<SetKey, (), RandomState>>,
    >(iter)
}

impl ConcurrentSet {
    fn new_inner() -> Arc<DashSet<SetKey>> {
        Arc::new(DashSet::new())
    }

    fn ensure_hashable(value: &Bound<'_, PyAny>) -> PyResult<SetKey> {
        SetKey::new(value)
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

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<ConcurrentSetIter>> {
        Py::new(py, ConcurrentSetIter::new(self.inner.clone()))
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
