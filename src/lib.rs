mod atomic;
mod dict;
mod locks;
mod queue;
mod set;
mod submodule;

use pyo3::prelude::*;
use shadow_rs::shadow;

shadow!(build);

const PY_VERSION: &str = env!("SYNCX_PY_VERSION");

#[pymodule(gil_used = false)]
fn syncx(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    atomic::register(py, m)?;
    dict::register(py, m)?;
    locks::register(py, m)?;
    queue::register(py, m)?;
    set::register(py, m)?;
    m.add("__version__", PY_VERSION)?;
    Ok(())
}
