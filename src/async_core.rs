use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::prettify::{prettify_document_html, prettify_fragment_html};
use crate::runtime::{future_into_py_tokio, spawn_blocking_py};
use crate::selectors::{
    select_first_with_limit, select_fragment, select_fragment_first, select_with_limit,
};
use crate::text::{
    attrs_from_element_html, inner_html_from_element_html, text_from_element_html,
    truncate_for_repr,
};
use crate::tl_dom::document_text;
use crate::xpath::{
    evaluate_fragment_xpath, evaluate_fragment_xpath_first, xpath_first_with_limit,
    xpath_with_limit,
};

#[derive(Clone)]
pub(crate) struct AsyncDocumentState {
    raw_html: Arc<str>,
    text: Arc<str>,
}

#[pyclass(name = "_AsyncElementCore", module = "scraper_rs.asyncio")]
pub struct AsyncElementCore {
    tag: String,
    outer_html: String,
    inner_html: OnceLock<String>,
    text: OnceLock<String>,
    attrs: OnceLock<HashMap<String, String>>,
}

impl From<Element> for AsyncElementCore {
    fn from(element: Element) -> Self {
        Self {
            tag: element.tag,
            outer_html: element.outer_html,
            inner_html: element.inner_html,
            text: element.text,
            attrs: element.attrs,
        }
    }
}

impl AsyncElementCore {
    pub(crate) fn wrap_many(elements: Vec<Element>) -> Vec<Self> {
        elements.into_iter().map(Self::from).collect()
    }

    pub(crate) fn wrap_one(element: Option<Element>) -> Option<Self> {
        element.map(Self::from)
    }
}

#[pymethods]
impl AsyncElementCore {
    #[getter]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    #[getter]
    pub fn text(&self) -> String {
        self.text
            .get_or_init(|| text_from_element_html(&self.outer_html))
            .clone()
    }

    #[getter]
    pub fn html(&self) -> &str {
        self.inner_html
            .get_or_init(|| inner_html_from_element_html(&self.outer_html))
    }

    #[getter]
    pub fn attrs(&self) -> HashMap<String, String> {
        self.attrs
            .get_or_init(|| attrs_from_element_html(&self.outer_html))
            .clone()
    }

    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .get_or_init(|| attrs_from_element_html(&self.outer_html))
            .get(name)
            .cloned()
    }

    pub fn get(&self, name: &str, default: Option<String>) -> Option<String> {
        self.attr(name).or(default)
    }

    pub fn select<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        let html = self.html().to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || select_fragment(&html, &css).map(AsyncElementCore::wrap_many))
                .await
        })
    }

    pub fn select_first<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        let html = self.html().to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                select_fragment_first(&html, &css).map(AsyncElementCore::wrap_one)
            })
            .await
        })
    }

    pub fn find<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        self.select_first(py, css)
    }

    pub fn css<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        self.select(py, css)
    }

    pub fn xpath<'py>(&self, py: Python<'py>, expr: String) -> PyResult<Bound<'py, PyAny>> {
        let html = self.html().to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                evaluate_fragment_xpath(&html, &expr).map(AsyncElementCore::wrap_many)
            })
            .await
        })
    }

    pub fn xpath_first<'py>(&self, py: Python<'py>, expr: String) -> PyResult<Bound<'py, PyAny>> {
        let html = self.html().to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                evaluate_fragment_xpath_first(&html, &expr).map(AsyncElementCore::wrap_one)
            })
            .await
        })
    }

    pub fn prettify<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let outer_html = self.outer_html.clone();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || prettify_fragment_html(&outer_html)).await
        })
    }

    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("tag", &self.tag)?;
        dict.set_item("text", self.text())?;
        dict.set_item("html", self.html())?;
        dict.set_item("attrs", self.attrs())?;
        Ok(dict.into())
    }

    fn __repr__(&self) -> String {
        let text_str = self.text();
        let text_preview = truncate_for_repr(text_str.trim(), 40);
        format!("<AsyncElement tag='{}' text={}>", self.tag, text_preview)
    }
}

#[pyclass(name = "_AsyncDocumentCore", module = "scraper_rs.asyncio")]
pub struct AsyncDocumentCore {
    state: Mutex<Option<AsyncDocumentState>>,
}

impl AsyncDocumentCore {
    fn new(raw_html: String, text: String) -> Self {
        Self {
            state: Mutex::new(Some(AsyncDocumentState {
                raw_html: Arc::<str>::from(raw_html),
                text: Arc::<str>::from(text),
            })),
        }
    }

    fn current_state(&self) -> Option<AsyncDocumentState> {
        self.state
            .lock()
            .expect("Async document state mutex poisoned")
            .clone()
    }

    pub(crate) fn from_html_input(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
        let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
        let raw_html = html_to_parse.into_owned();
        let text = {
            let parsed = tl::parse(&raw_html, tl::ParserOptions::default())
                .map_err(|err| PyValueError::new_err(err.to_string()))?;
            document_text(&parsed)
        };
        Ok(Self::new(raw_html, text))
    }
}

#[pymethods]
impl AsyncDocumentCore {
    #[getter]
    pub fn html(&self) -> String {
        self.current_state()
            .map(|state| state.raw_html.as_ref().to_string())
            .unwrap_or_default()
    }

    #[getter]
    pub fn text(&self) -> String {
        self.current_state()
            .map(|state| state.text.as_ref().to_string())
            .unwrap_or_default()
    }

    pub fn select<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        let Some(state) = self.current_state() else {
            return future_into_py_tokio(py, async { Ok(Vec::<AsyncElementCore>::new()) });
        };
        let html = state.raw_html.to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                select_with_limit(&html, &css, Some(html.len()), false)
                    .map(AsyncElementCore::wrap_many)
            })
            .await
        })
    }

    pub fn select_first<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        let Some(state) = self.current_state() else {
            return future_into_py_tokio(py, async { Ok(None::<AsyncElementCore>) });
        };
        let html = state.raw_html.to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                select_first_with_limit(&html, &css, Some(html.len()), false)
                    .map(AsyncElementCore::wrap_one)
            })
            .await
        })
    }

    pub fn find<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        self.select_first(py, css)
    }

    pub fn css<'py>(&self, py: Python<'py>, css: String) -> PyResult<Bound<'py, PyAny>> {
        self.select(py, css)
    }

    pub fn xpath<'py>(&self, py: Python<'py>, expr: String) -> PyResult<Bound<'py, PyAny>> {
        let Some(state) = self.current_state() else {
            return future_into_py_tokio(py, async { Ok(Vec::<AsyncElementCore>::new()) });
        };
        let html = state.raw_html.to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                xpath_with_limit(&html, &expr, Some(html.len()), false)
                    .map(AsyncElementCore::wrap_many)
            })
            .await
        })
    }

    pub fn xpath_first<'py>(&self, py: Python<'py>, expr: String) -> PyResult<Bound<'py, PyAny>> {
        let Some(state) = self.current_state() else {
            return future_into_py_tokio(py, async { Ok(None::<AsyncElementCore>) });
        };
        let html = state.raw_html.to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || {
                xpath_first_with_limit(&html, &expr, Some(html.len()), false)
                    .map(AsyncElementCore::wrap_one)
            })
            .await
        })
    }

    pub fn prettify<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let Some(state) = self.current_state() else {
            return future_into_py_tokio(py, async { Ok(String::new()) });
        };
        let html = state.raw_html.to_string();
        future_into_py_tokio(py, async move {
            spawn_blocking_py(move || Ok(prettify_document_html(&html))).await
        })
    }

    pub fn close(&self) {
        *self
            .state
            .lock()
            .expect("Async document state mutex poisoned") = None;
    }

    fn __repr__(&self) -> String {
        format!("<AsyncDocument len_html={}>", self.html().len())
    }
}
