mod async_core;
mod cache;
mod document;
mod element;
mod functions;
mod html5_dict;
mod limits;
mod prettify;
mod runtime;
mod selectors;
mod text;
mod xpath;

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

use async_core::{AsyncDocumentCore, AsyncElementCore};
use document::Document;
use element::Element;
use functions::{
    first, first_async, parse, parse_async, parse_document_dict, parse_fragment_dict,
    prettify_async, select, select_async, select_first, select_first_async, xpath_async,
    xpath_first, xpath_first_async,
};

/// Top-level module initializer.
#[pymodule(gil_used = false)]
fn scraper_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    pyo3_async_runtimes::tokio::init(builder);

    // Classes
    m.add_class::<Document>()?;
    m.add_class::<Element>()?;
    m.add_class::<AsyncDocumentCore>()?;
    m.add_class::<AsyncElementCore>()?;

    // Top-level functions
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(parse_document_dict, m)?)?;
    m.add_function(wrap_pyfunction!(parse_fragment_dict, m)?)?;
    m.add_function(wrap_pyfunction!(functions::prettify, m)?)?;
    m.add_function(wrap_pyfunction!(select, m)?)?;
    m.add_function(wrap_pyfunction!(select_first, m)?)?;
    m.add_function(wrap_pyfunction!(first, m)?)?;
    m.add_function(wrap_pyfunction!(functions::xpath, m)?)?;
    m.add_function(wrap_pyfunction!(xpath_first, m)?)?;

    // Async versions
    m.add_function(wrap_pyfunction!(parse_async, m)?)?;
    m.add_function(wrap_pyfunction!(prettify_async, m)?)?;
    m.add_function(wrap_pyfunction!(select_async, m)?)?;
    m.add_function(wrap_pyfunction!(select_first_async, m)?)?;
    m.add_function(wrap_pyfunction!(first_async, m)?)?;
    m.add_function(wrap_pyfunction!(xpath_async, m)?)?;
    m.add_function(wrap_pyfunction!(xpath_first_async, m)?)?;

    // Package metadata
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}

#[cfg(test)]
mod tests;
