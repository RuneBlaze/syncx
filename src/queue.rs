use std::time::Duration;

use crate::submodule;
use flume::{Receiver, RecvTimeoutError, Sender, TryRecvError, TrySendError};
use flume::{SendError, SendTimeoutError};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyAny;

pyo3::create_exception!(queue_module, Empty, pyo3::exceptions::PyException);
pyo3::create_exception!(queue_module, Full, pyo3::exceptions::PyException);

pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "queue")?;
    module.add_class::<Queue>()?;
    module.add("Empty", py.get_type::<Empty>())?;
    module.add("Full", py.get_type::<Full>())?;
    submodule::register_submodule(py, parent, &module, "syncx.queue")?;
    Ok(())
}

#[pyclass(module = "syncx.queue")]
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
