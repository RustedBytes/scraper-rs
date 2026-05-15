use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::async_core::{AsyncDocumentCore, AsyncElementCore};
use crate::document::Document;
use crate::element::Element;
use crate::html5_dict::{
    html5_tree_to_py_dict, parse_document_to_dict_tree, parse_fragment_to_dict_tree,
};
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::prettify::prettify_document_html;
use crate::runtime::{future_into_py_tokio, spawn_blocking_py};
use crate::selectors::{select_first_with_limit, select_with_limit};
use crate::xpath::{xpath_first_with_limit, xpath_with_limit};

#[pyfunction]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn parse(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Document> {
    Document::from_html(html, max_size_bytes, truncate_on_limit)
}

#[pyfunction(name = "parse_document")]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn parse_document_dict(
    py: Python<'_>,
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Py<PyDict>> {
    let tree = parse_document_to_dict_tree(html, max_size_bytes, truncate_on_limit)?;
    html5_tree_to_py_dict(py, tree)
}

#[pyfunction(name = "parse_fragment")]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn parse_fragment_dict(
    py: Python<'_>,
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Py<PyDict>> {
    let tree = parse_fragment_to_dict_tree(html, max_size_bytes, truncate_on_limit)?;
    html5_tree_to_py_dict(py, tree)
}

#[pyfunction]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn prettify(
    py: Python<'_>,
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<String> {
    py.detach(|| {
        let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
        let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
        Ok(prettify_document_html(html_to_parse.as_ref()))
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn select(
    py: Python<'_>,
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    py.detach(|| select_with_limit(html, css, max_size_bytes, truncate_on_limit))
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn select_first(
    py: Python<'_>,
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    py.detach(|| select_first_with_limit(html, css, max_size_bytes, truncate_on_limit))
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn first(
    py: Python<'_>,
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    py.detach(|| select_first_with_limit(html, css, max_size_bytes, truncate_on_limit))
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn xpath(
    py: Python<'_>,
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    py.detach(|| xpath_with_limit(html, expr, max_size_bytes, truncate_on_limit))
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn xpath_first(
    py: Python<'_>,
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    py.detach(|| xpath_first_with_limit(html, expr, max_size_bytes, truncate_on_limit))
}

// Async versions using pyo3-async-runtimes

#[pyfunction]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn parse_async(
    py: Python<'_>,
    html: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            AsyncDocumentCore::from_html_input(&html, max_size_bytes, truncate_on_limit)
        })
        .await
    })
}

#[pyfunction]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn prettify_async(
    py: Python<'_>,
    html: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
            let html_to_parse = ensure_within_size_limit(&html, max_size_bytes, truncate_on_limit)?;
            Ok(prettify_document_html(html_to_parse.as_ref()))
        })
        .await
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn select_async(
    py: Python<'_>,
    html: String,
    css: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            select_with_limit(&html, &css, max_size_bytes, truncate_on_limit)
                .map(AsyncElementCore::wrap_many)
        })
        .await
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn select_first_async(
    py: Python<'_>,
    html: String,
    css: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            select_first_with_limit(&html, &css, max_size_bytes, truncate_on_limit)
                .map(AsyncElementCore::wrap_one)
        })
        .await
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn first_async(
    py: Python<'_>,
    html: String,
    css: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            select_first_with_limit(&html, &css, max_size_bytes, truncate_on_limit)
                .map(AsyncElementCore::wrap_one)
        })
        .await
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn xpath_async(
    py: Python<'_>,
    html: String,
    expr: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            xpath_with_limit(&html, &expr, max_size_bytes, truncate_on_limit)
                .map(AsyncElementCore::wrap_many)
        })
        .await
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
pub(crate) fn xpath_first_async(
    py: Python<'_>,
    html: String,
    expr: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    future_into_py_tokio(py, async move {
        spawn_blocking_py(move || {
            xpath_first_with_limit(&html, &expr, max_size_bytes, truncate_on_limit)
                .map(AsyncElementCore::wrap_one)
        })
        .await
    })
}
