use crate::submodule;
use parking_lot::lock_api::{
    RawMutex as RawMutexTrait, RawRwLock as RawRwLockTrait, RawRwLockDowngrade,
};
use parking_lot::{RawMutex, RawRwLock, ReentrantMutex, ReentrantMutexGuard};
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::Bound;
use std::mem::transmute;
use std::sync::Arc;

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
#[derive(Clone)]
pub struct Lock {
    inner: Arc<RawMutex>,
}

#[pymethods]
impl Lock {
    #[new]
    fn new() -> Self {
        Self {
            inner: Arc::new(RawMutex::INIT),
        }
    }

    pub fn acquire(&self) -> LockGuard {
        self.inner.lock();
        LockGuard {
            guard: LockGuardState::Locked(Arc::clone(&self.inner)),
        }
    }

    #[pyo3(name = "lock")]
    pub fn lock_alias(&self) -> LockGuard {
        self.acquire()
    }

    pub fn try_acquire(&self) -> Option<LockGuard> {
        if self.inner.try_lock() {
            Some(LockGuard {
                guard: LockGuardState::Locked(Arc::clone(&self.inner)),
            })
        } else {
            None
        }
    }

    #[pyo3(name = "try_lock")]
    pub fn try_lock_alias(&self) -> Option<LockGuard> {
        self.try_acquire()
    }

    pub fn locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub fn is_locked(&self) -> bool {
        self.locked()
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        slf.inner.lock();
        Ok(slf)
    }

    fn __exit__(
        &self,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        unsafe {
            self.inner.unlock();
        }
        Ok(false)
    }
}

#[pyclass(module = "syncx.locks")]
pub struct LockGuard {
    guard: LockGuardState,
}

enum LockGuardState {
    Locked(Arc<RawMutex>),
    Released,
}

#[pymethods]
impl LockGuard {
    pub fn release(&mut self) {
        if let LockGuardState::Locked(mutex) = &self.guard {
            unsafe {
                mutex.unlock();
            }
            self.guard = LockGuardState::Released;
        }
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
        if let LockGuardState::Locked(mutex) = &self.guard {
            unsafe {
                mutex.unlock();
            }
            self.guard = LockGuardState::Released;
        }
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

#[pyclass(module = "syncx.locks", unsendable)]
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
#[derive(Clone)]
pub struct RWLock {
    inner: Arc<RawRwLock>,
}

#[pymethods]
impl RWLock {
    #[new]
    fn new() -> Self {
        Self {
            inner: Arc::new(RawRwLock::INIT),
        }
    }

    pub fn acquire_read(&self) -> ReadGuard {
        self.inner.lock_shared();
        ReadGuard {
            guard: GuardState::ReadLocked(Arc::clone(&self.inner)),
        }
    }

    #[pyo3(name = "read_lock")]
    pub fn read_lock_alias(&self) -> ReadGuard {
        self.acquire_read()
    }

    pub fn try_acquire_read(&self) -> Option<ReadGuard> {
        if self.inner.try_lock_shared() {
            Some(ReadGuard {
                guard: GuardState::ReadLocked(Arc::clone(&self.inner)),
            })
        } else {
            None
        }
    }

    #[pyo3(name = "try_read_lock")]
    pub fn try_read_lock_alias(&self) -> Option<ReadGuard> {
        self.try_acquire_read()
    }

    pub fn acquire_write(&self) -> WriteGuard {
        self.inner.lock_exclusive();
        WriteGuard {
            guard: WriteGuardState::WriteLocked(Arc::clone(&self.inner)),
        }
    }

    #[pyo3(name = "write_lock")]
    pub fn write_lock_alias(&self) -> WriteGuard {
        self.acquire_write()
    }

    pub fn try_acquire_write(&self) -> Option<WriteGuard> {
        if self.inner.try_lock_exclusive() {
            Some(WriteGuard {
                guard: WriteGuardState::WriteLocked(Arc::clone(&self.inner)),
            })
        } else {
            None
        }
    }

    #[pyo3(name = "try_write_lock")]
    pub fn try_write_lock_alias(&self) -> Option<WriteGuard> {
        self.try_acquire_write()
    }

    pub fn is_locked(&self) -> bool {
        self.inner.is_locked()
    }

    pub fn is_write_locked(&self) -> bool {
        self.inner.is_locked_exclusive()
    }
}

#[pyclass(module = "syncx.locks")]
pub struct ReadGuard {
    guard: GuardState,
}

enum GuardState {
    ReadLocked(Arc<RawRwLock>),
    Released,
}

#[pymethods]
impl ReadGuard {
    pub fn release(&mut self) {
        if let GuardState::ReadLocked(lock) = &self.guard {
            unsafe {
                lock.unlock_shared();
            }
            self.guard = GuardState::Released;
        }
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
        if let GuardState::ReadLocked(lock) = &self.guard {
            unsafe {
                lock.unlock_shared();
            }
            self.guard = GuardState::Released;
        }
    }
}

#[pyclass(module = "syncx.locks")]
pub struct WriteGuard {
    guard: WriteGuardState,
}

enum WriteGuardState {
    WriteLocked(Arc<RawRwLock>),
    Released,
}

#[pymethods]
impl WriteGuard {
    pub fn release(&mut self) {
        if let WriteGuardState::WriteLocked(lock) = &self.guard {
            unsafe {
                lock.unlock_exclusive();
            }
            self.guard = WriteGuardState::Released;
        }
    }

    #[pyo3(name = "unlock")]
    pub fn unlock_alias(&mut self) {
        self.release();
    }

    pub fn downgrade(&mut self) -> Option<ReadGuard> {
        match std::mem::replace(&mut self.guard, WriteGuardState::Released) {
            WriteGuardState::WriteLocked(lock) => {
                unsafe {
                    lock.downgrade();
                }
                let read_arc = Arc::clone(&lock);
                Some(ReadGuard {
                    guard: GuardState::ReadLocked(read_arc),
                })
            }
            WriteGuardState::Released => None,
        }
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
        if let WriteGuardState::WriteLocked(lock) = &self.guard {
            unsafe {
                lock.unlock_exclusive();
            }
            self.guard = WriteGuardState::Released;
        }
    }
}
