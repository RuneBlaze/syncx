use crate::submodule;
use parking_lot::lock_api::{
    RawMutex as RawMutexTrait, RawMutexTimed, RawRwLock as RawRwLockTrait, RawRwLockDowngrade,
    RawRwLockFair, RawRwLockTimed,
};
use parking_lot::{RawMutex, RawRwLock, ReentrantMutex, ReentrantMutexGuard};
use pyo3::conversion::IntoPyObject;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::Bound;
use std::mem::transmute;
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[allow(deprecated)]
fn lock_with_options(
    inner: &RawMutex,
    py: Python<'_>,
    blocking: bool,
    timeout: Option<f64>,
) -> PyResult<bool> {
    if !blocking {
        return Ok(inner.try_lock());
    }

    if inner.try_lock() {
        return Ok(true);
    }

    match timeout {
        None => {
            py.allow_threads(|| inner.lock());
            Ok(true)
        }
        Some(value) => {
            if value.is_sign_negative() {
                return Ok(false);
            }
            if !value.is_finite() {
                py.allow_threads(|| inner.lock());
                return Ok(true);
            }

            let max_secs = Duration::MAX.as_secs_f64();
            if value >= max_secs {
                py.allow_threads(|| inner.lock());
                return Ok(true);
            }

            let duration = Duration::from_secs_f64(value);
            let deadline = Instant::now()
                .checked_add(duration)
                .unwrap_or_else(Instant::now);
            Ok(py.allow_threads(|| inner.try_lock_until(deadline)))
        }
    }
}

#[allow(deprecated)]
fn lock_shared_with_options(
    inner: &RawRwLock,
    py: Python<'_>,
    blocking: bool,
    timeout: Option<f64>,
) -> PyResult<bool> {
    if !blocking {
        return Ok(inner.try_lock_shared());
    }

    if inner.try_lock_shared() {
        return Ok(true);
    }

    match timeout {
        None => {
            py.allow_threads(|| inner.lock_shared());
            Ok(true)
        }
        Some(value) => {
            if value.is_sign_negative() {
                return Ok(false);
            }
            if !value.is_finite() {
                py.allow_threads(|| inner.lock_shared());
                return Ok(true);
            }

            let max_secs = Duration::MAX.as_secs_f64();
            if value >= max_secs {
                py.allow_threads(|| inner.lock_shared());
                return Ok(true);
            }

            let duration = Duration::from_secs_f64(value);
            let deadline = Instant::now()
                .checked_add(duration)
                .unwrap_or_else(Instant::now);
            Ok(py.allow_threads(|| inner.try_lock_shared_until(deadline)))
        }
    }
}

#[allow(deprecated)]
fn lock_exclusive_with_options(
    inner: &RawRwLock,
    py: Python<'_>,
    blocking: bool,
    timeout: Option<f64>,
) -> PyResult<bool> {
    if !blocking {
        return Ok(inner.try_lock_exclusive());
    }

    if inner.try_lock_exclusive() {
        return Ok(true);
    }

    match timeout {
        None => {
            py.allow_threads(|| inner.lock_exclusive());
            Ok(true)
        }
        Some(value) => {
            if value.is_sign_negative() {
                return Ok(false);
            }
            if !value.is_finite() {
                py.allow_threads(|| inner.lock_exclusive());
                return Ok(true);
            }

            let max_secs = Duration::MAX.as_secs_f64();
            if value >= max_secs {
                py.allow_threads(|| inner.lock_exclusive());
                return Ok(true);
            }

            let duration = Duration::from_secs_f64(value);
            let deadline = Instant::now()
                .checked_add(duration)
                .unwrap_or_else(Instant::now);
            Ok(py.allow_threads(|| inner.try_lock_exclusive_until(deadline)))
        }
    }
}

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "locks")?;
    module.add_class::<Lock>()?;
    module.add_class::<LockGuard>()?;
    module.add_class::<RLock>()?;
    module.add_class::<RLockGuard>()?;
    module.add_class::<RWLock>()?;
    module.add_class::<ReadGuard>()?;
    module.add_class::<WriteGuard>()?;
    submodule::register_submodule(py, parent, &module, "syncx.locks")?;
    Ok(())
}

#[pyclass(module = "syncx.locks")]
pub struct Lock {
    inner: RawMutex,
}

#[pymethods]
impl Lock {
    #[new]
    fn new() -> Self {
        Self {
            inner: RawMutex::INIT,
        }
    }

    #[pyo3(signature = (blocking=true, timeout=None))]
    pub fn acquire(&self, py: Python<'_>, blocking: bool, timeout: Option<f64>) -> PyResult<bool> {
        lock_with_options(&self.inner, py, blocking, timeout)
    }

    #[pyo3(name = "lock", signature = (blocking=true, timeout=None))]
    pub fn lock_alias(
        &self,
        py: Python<'_>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<bool> {
        self.acquire(py, blocking, timeout)
    }

    pub fn try_acquire(&self) -> bool {
        self.inner.try_lock()
    }

    #[pyo3(name = "try_lock")]
    pub fn try_lock_alias(&self) -> bool {
        self.try_acquire()
    }

    pub fn release(&self) {
        unsafe {
            self.inner.unlock();
        }
    }

    pub fn locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub fn is_locked(&self) -> bool {
        self.locked()
    }

    #[pyo3(signature = (blocking=true, timeout=None))]
    pub fn guard<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<Option<LockGuard>> {
        if !lock_with_options(&slf.inner, py, blocking, timeout)? {
            return Ok(None);
        }
        let ptr = NonNull::from(&slf.inner);
        let owner = slf.into_pyobject(py)?.unbind().into_any();
        Ok(Some(LockGuard::new(owner, ptr)))
    }

    fn __enter__<'py>(slf: PyRef<'py, Self>, py: Python<'py>) -> PyResult<PyRef<'py, Self>> {
        slf.acquire(py, true, None)?;
        Ok(slf)
    }

    fn __exit__(
        &self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.release();
        Ok(false)
    }
}

#[pyclass(module = "syncx.locks", unsendable, freelist = 4096)]
pub struct LockGuard {
    _owner: Py<PyAny>,
    ptr: NonNull<RawMutex>,
    held: bool,
}

impl LockGuard {
    fn new(owner: Py<PyAny>, ptr: NonNull<RawMutex>) -> Self {
        Self {
            _owner: owner,
            ptr,
            held: true,
        }
    }

    fn unlock_raw(&mut self) {
        if self.held {
            unsafe {
                self.ptr.as_ref().unlock();
            }
            self.held = false;
        }
    }
}

#[pymethods]
impl LockGuard {
    pub fn release(&mut self) {
        self.unlock_raw();
    }

    #[pyo3(name = "unlock")]
    pub fn unlock_alias(&mut self) {
        self.release();
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.release();
        Ok(false)
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        self.unlock_raw();
    }
}

#[pyclass(module = "syncx.locks")]
#[derive(Clone)]
pub struct RLock {
    inner: Arc<ReentrantMutex<()>>,
}

#[pymethods]
impl RLock {
    #[new]
    fn new() -> Self {
        Self {
            inner: Arc::new(ReentrantMutex::new(())),
        }
    }

    pub fn acquire(&self) -> RLockGuard {
        let guard = self.inner.lock();
        RLockGuard::new(Arc::clone(&self.inner), guard)
    }

    #[pyo3(name = "lock")]
    pub fn lock_alias(&self) -> RLockGuard {
        self.acquire()
    }

    pub fn try_acquire(&self) -> Option<RLockGuard> {
        self.inner
            .try_lock()
            .map(|guard| RLockGuard::new(Arc::clone(&self.inner), guard))
    }

    #[pyo3(name = "try_lock")]
    pub fn try_lock_alias(&self) -> Option<RLockGuard> {
        self.try_acquire()
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<RLockGuard> {
        Ok(slf.acquire())
    }

    fn __exit__(
        &self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        Ok(false)
    }
}

#[pyclass(module = "syncx.locks", unsendable, freelist = 128)]
pub struct RLockGuard {
    guard: Option<ReentrantMutexGuard<'static, ()>>,
    _lock: Arc<ReentrantMutex<()>>,
}

impl RLockGuard {
    fn new(lock: Arc<ReentrantMutex<()>>, guard: ReentrantMutexGuard<'_, ()>) -> Self {
        // SAFETY: we store the underlying mutex inside the guard to keep it alive for the
        // extended lifetime. The guard is dropped before the Arc in Drop, preserving order.
        let guard_static: ReentrantMutexGuard<'static, ()> = unsafe { transmute(guard) };
        Self {
            guard: Some(guard_static),
            _lock: lock,
        }
    }
}

#[pymethods]
impl RLockGuard {
    pub fn release(&mut self) {
        self.guard.take();
    }

    #[pyo3(name = "unlock")]
    pub fn unlock_alias(&mut self) {
        self.release();
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.release();
        Ok(false)
    }
}

impl Drop for RLockGuard {
    fn drop(&mut self) {
        self.guard.take();
    }
}

#[pyclass(module = "syncx.locks")]
pub struct RWLock {
    inner: RawRwLock,
}

#[pymethods]
impl RWLock {
    #[new]
    fn new() -> Self {
        Self {
            inner: RawRwLock::INIT,
        }
    }

    #[pyo3(signature = (blocking=true, timeout=None))]
    pub fn acquire_read(
        &self,
        py: Python<'_>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<bool> {
        lock_shared_with_options(&self.inner, py, blocking, timeout)
    }

    #[pyo3(name = "read_lock", signature = (blocking=true, timeout=None))]
    pub fn read_lock_alias(
        &self,
        py: Python<'_>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<bool> {
        self.acquire_read(py, blocking, timeout)
    }

    pub fn read_release(&self) {
        unsafe {
            self.inner.unlock_shared();
        }
    }

    pub fn try_acquire_read(&self) -> bool {
        self.inner.try_lock_shared()
    }

    #[pyo3(name = "try_read_lock")]
    pub fn try_read_lock_alias(&self) -> bool {
        self.try_acquire_read()
    }

    #[pyo3(signature = (blocking=true, timeout=None))]
    pub fn read_guard<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<Option<ReadGuard>> {
        if !lock_shared_with_options(&slf.inner, py, blocking, timeout)? {
            return Ok(None);
        }
        let ptr = NonNull::from(&slf.inner);
        let owner = slf.into_pyobject(py)?.unbind().into_any();
        Ok(Some(ReadGuard::new(owner, ptr)))
    }

    #[pyo3(signature = (blocking=true, timeout=None))]
    pub fn acquire_write(
        &self,
        py: Python<'_>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<bool> {
        lock_exclusive_with_options(&self.inner, py, blocking, timeout)
    }

    #[pyo3(name = "write_lock", signature = (blocking=true, timeout=None))]
    pub fn write_lock_alias(
        &self,
        py: Python<'_>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<bool> {
        self.acquire_write(py, blocking, timeout)
    }

    pub fn write_release(&self) {
        unsafe {
            self.inner.unlock_exclusive();
        }
    }

    #[pyo3(signature = (blocking=true, timeout=None))]
    pub fn write_guard<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
        blocking: bool,
        timeout: Option<f64>,
    ) -> PyResult<Option<WriteGuard>> {
        if !lock_exclusive_with_options(&slf.inner, py, blocking, timeout)? {
            return Ok(None);
        }
        let ptr = NonNull::from(&slf.inner);
        let owner = slf.into_pyobject(py)?.unbind().into_any();
        Ok(Some(WriteGuard::new(owner, ptr)))
    }

    pub fn try_acquire_write(&self) -> bool {
        self.inner.try_lock_exclusive()
    }

    #[pyo3(name = "try_write_lock")]
    pub fn try_write_lock_alias(&self) -> bool {
        self.try_acquire_write()
    }

    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub fn is_write_locked(&self) -> bool {
        self.inner.is_locked_exclusive()
    }

    pub fn write_release_fair(&self) {
        unsafe {
            self.inner.unlock_exclusive_fair();
        }
    }

    pub fn bump_shared(&self) {
        unsafe {
            self.inner.bump_shared();
        }
    }

    pub fn bump_exclusive(&self) {
        unsafe {
            self.inner.bump_exclusive();
        }
    }
}

#[pyclass(module = "syncx.locks", unsendable, freelist = 4096)]
pub struct ReadGuard {
    _owner: Py<PyAny>,
    ptr: NonNull<RawRwLock>,
    held: bool,
}

impl ReadGuard {
    fn new(owner: Py<PyAny>, ptr: NonNull<RawRwLock>) -> Self {
        Self {
            _owner: owner,
            ptr,
            held: true,
        }
    }

    fn unlock_raw(&mut self) {
        if self.held {
            unsafe {
                self.ptr.as_ref().unlock_shared();
            }
            self.held = false;
        }
    }
}

#[pymethods]
impl ReadGuard {
    pub fn release(&mut self) {
        self.unlock_raw();
    }

    #[pyo3(name = "unlock")]
    pub fn unlock_alias(&mut self) {
        self.release();
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.release();
        Ok(false)
    }
}

impl Drop for ReadGuard {
    fn drop(&mut self) {
        self.unlock_raw();
    }
}

#[pyclass(module = "syncx.locks", unsendable, freelist = 4096)]
pub struct WriteGuard {
    _owner: Py<PyAny>,
    ptr: NonNull<RawRwLock>,
    exclusive: bool,
}

impl WriteGuard {
    fn new(owner: Py<PyAny>, ptr: NonNull<RawRwLock>) -> Self {
        Self {
            _owner: owner,
            ptr,
            exclusive: true,
        }
    }

    fn unlock_raw(&mut self) {
        if self.exclusive {
            unsafe {
                self.ptr.as_ref().unlock_exclusive();
            }
            self.exclusive = false;
        }
    }

    fn unlock_raw_fair(&mut self) {
        if self.exclusive {
            unsafe {
                self.ptr.as_ref().unlock_exclusive_fair();
            }
            self.exclusive = false;
        }
    }
}

#[pymethods]
impl WriteGuard {
    pub fn release(&mut self) {
        self.unlock_raw();
    }

    #[pyo3(name = "unlock")]
    pub fn unlock_alias(&mut self) {
        self.release();
    }

    pub fn release_fair(&mut self) {
        self.unlock_raw_fair();
    }

    #[pyo3(name = "unlock_fair")]
    pub fn unlock_fair_alias(&mut self) {
        self.release_fair();
    }

    pub fn downgrade(&mut self, py: Python<'_>) -> Option<ReadGuard> {
        if !self.exclusive {
            return None;
        }
        unsafe {
            self.ptr.as_ref().downgrade();
        }
        self.exclusive = false;
        let owner = self._owner.clone_ref(py);
        Some(ReadGuard::new(owner, self.ptr))
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        self.release();
        Ok(false)
    }
}

impl Drop for WriteGuard {
    fn drop(&mut self) {
        self.unlock_raw();
    }
}
