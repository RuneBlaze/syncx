mod atomic;
mod collections;
mod locks;
mod submodule;

use pyo3::prelude::*;
use shadow_rs::shadow;

shadow!(build);

const PY_VERSION: &str = env!("SYNCX_PY_VERSION");

#[pymodule(gil_used = false)]
fn syncx(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    atomic::register(py, m)?;
    collections::register(py, m)?;
    locks::register(py, m)?;
    m.add("__version__", PY_VERSION)?;
    Ok(())
}
