use std::future::Future;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(crate) fn task_locals(py: Python<'_>) -> PyResult<pyo3_async_runtimes::TaskLocals> {
    pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)
}

pub(crate) fn future_into_py_tokio<F, T>(py: Python<'_>, fut: F) -> PyResult<Bound<'_, PyAny>>
where
    F: Future<Output = PyResult<T>> + Send + 'static,
    T: for<'py> IntoPyObject<'py> + Send + 'static,
{
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, task_locals(py)?, fut)
}

pub(crate) async fn spawn_blocking_py<F, T>(work: F) -> PyResult<T>
where
    F: FnOnce() -> PyResult<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
}
