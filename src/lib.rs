mod atomic;
mod dict;
mod locks;
mod queue;
mod submodule;

use pyo3::prelude::*;

#[pymodule(gil_used = false)]
fn syncx(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    atomic::register(py, m)?;
    dict::register(py, m)?;
    locks::register(py, m)?;
    queue::register(py, m)?;
    Ok(())
}
