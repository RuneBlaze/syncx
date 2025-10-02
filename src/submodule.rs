use pyo3::prelude::*;
use pyo3::types::PyDict;

pub fn register_submodule(
    py: Python<'_>,
    parent: &Bound<'_, PyModule>,
    module: &Bound<'_, PyModule>,
    qualname: &str,
) -> PyResult<()> {
    module.gil_used(false)?;
    parent.add_submodule(module)?;
    let sys = py.import("sys")?;
    let modules: Bound<'_, PyDict> = sys.getattr("modules")?.downcast_into()?;
    modules.set_item(qualname, module)?;
    Ok(())
}
