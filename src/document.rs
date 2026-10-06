use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::element::Element;
use crate::prettify::prettify_document_html;
use crate::selector_generator::{
    generate_absolute_xpath_selector, generate_css_selector, generate_robust_xpath_candidates,
};
use crate::tl_dom::{
    OwnedTlDom, document_text, parse_owned_html_unlimited, parse_owned_html_with_raw,
    select_elements_from_dom_with_source, select_first_element_from_dom_with_source,
};
use crate::xpath::{
    XPathDocumentState, evaluate_xpath_elements, evaluate_xpath_first_element,
    find_unique_xpath_selector, parse_xpath_documents,
};

static NEXT_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

/// A parsed HTML document with convenient, Pythonic selectors.
///
/// Example:
///
///     from scraper_rs import Document
///
///     doc = Document("<html><body><a href='/x'>link</a></body></html>")
///     a = doc.find("a")
///     print(a.text, a.attr("href"))
#[pyclass(module = "scraper_rs", unsendable)]
pub struct Document {
    dom: OwnedTlDom,
    document_id: u64,
    xpath_state: Mutex<Option<XPathDocumentState>>,
    closed: bool,
}

impl Document {
    pub(crate) fn parse_with_limit(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        let dom = parse_owned_html_with_raw(html, max_size_bytes, truncate_on_limit)?;

        Ok(Self {
            dom,
            document_id: NEXT_DOCUMENT_ID.fetch_add(1, Ordering::Relaxed),
            xpath_state: Mutex::new(None),
            closed: false,
        })
    }

    /// Get or initialize the `XPath` state lazily.
    ///
    /// Panics if the mutex is poisoned (only happens if a panic occurred
    /// while holding the lock, which should not happen in normal operation).
    fn ensure_xpath_state(
        &self,
    ) -> PyResult<std::sync::MutexGuard<'_, Option<XPathDocumentState>>> {
        let mut state_lock = self.xpath_state.lock().expect("XPath state mutex poisoned");

        // Check if already initialized
        if state_lock.is_none() {
            let (documents, document_handle) =
                parse_xpath_documents(self.dom.html(), "HTML document")?;
            *state_lock = Some(XPathDocumentState {
                documents,
                document_handle,
            });
        }

        Ok(state_lock)
    }

    /// Drop all DOM allocations and shrink owned strings.
    fn release_dom(&mut self) {
        if self.closed {
            return;
        }

        self.dom =
            parse_owned_html_unlimited(String::new()).expect("empty HTML should parse with tl");
        // Mutex should never be poisoned here, but use expect for better error message
        *self.xpath_state.lock().expect("XPath state mutex poisoned") = None;
        self.closed = true;
    }
}

#[pymethods]
impl Document {
    /// Create a Document from a raw HTML string.
    ///
    ///     doc = Document("<html>...</html>")
    ///
    /// # Errors
    ///
    /// Returns an error if the HTML exceeds `max_size_bytes` and truncation is disabled.
    #[new]
    #[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
    pub fn new(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        Self::parse_with_limit(html, max_size_bytes, truncate_on_limit)
    }

    /// Alternate constructor: `Document.from_html(html: str) -> Document`
    ///
    /// # Errors
    ///
    /// Returns an error if the HTML exceeds `max_size_bytes` and truncation is disabled.
    #[staticmethod]
    #[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
    pub fn from_html(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        Self::parse_with_limit(html, max_size_bytes, truncate_on_limit)
    }

    /// Return the original HTML string.
    #[getter]
    pub fn html(&self) -> &str {
        self.dom.html()
    }

    /// All text content from the document, normalized and joined by spaces.
    #[getter]
    pub fn text(&self) -> String {
        if self.closed {
            return String::new();
        }
        document_text(self.dom.get_ref())
    }

    /// Return the current document HTML formatted with indentation.
    ///
    /// # Errors
    ///
    /// This currently does not return errors, but the `PyResult` is preserved for API consistency.
    pub fn prettify(&self) -> PyResult<String> {
        Ok(prettify_document_html(self.dom.html()))
    }

    /// Select all elements matching the given CSS selector.
    ///
    /// Returns a list[Element].
    ///
    ///     links = doc.select("a[href]")
    ///     for el in links:
    ///         print(el.text, el.attr("href"))
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn select(&self, css: &str) -> PyResult<Vec<Element>> {
        if self.closed {
            return Ok(Vec::new());
        }
        select_elements_from_dom_with_source(self.dom.get_ref(), css, self.document_id)
    }

    /// Return the first matching element, or None if nothing matches.
    ///
    ///     first_link = doc.select_first("a[href]")
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn select_first(&self, css: &str) -> PyResult<Option<Element>> {
        if self.closed {
            return Ok(None);
        }
        select_first_element_from_dom_with_source(self.dom.get_ref(), css, self.document_id)
    }

    /// Return the first matching element, or None if nothing matches.
    ///
    ///     first_link = doc.find("a[href]")
    ///     if first_link:
    ///         print(first_link.text)
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn find(&self, css: &str) -> PyResult<Option<Element>> {
        self.select_first(css)
    }

    /// Shorthand for `select(css)`; more “requests-html” style.
    ///
    ///     doc.css("div.item")
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn css(&self, css: &str) -> PyResult<Vec<Element>> {
        self.select(css)
    }

    /// Generate a unique CSS selector for an element returned by this document's CSS API.
    ///
    /// Returns None if the bounded search finds no supported unique CSS selector.
    /// This does not prove that no such CSS selector exists. Elements from another document or a
    /// detached/nested element snapshot are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error if the element is not attached to this document.
    pub fn generate_css_selector(&self, element: PyRef<'_, Element>) -> PyResult<Option<String>> {
        if self.closed {
            return Ok(None);
        }
        let source = element.source.ok_or_else(|| {
            PyValueError::new_err(
                "Selector generation requires an Element returned by this Document's CSS selection API",
            )
        })?;
        if source.document_id != self.document_id {
            return Err(PyValueError::new_err(
                "Element belongs to a different Document",
            ));
        }

        Ok(generate_css_selector(self.dom.get_ref(), source.handle))
    }

    /// Generate a unique XPath selector for an element returned by this document's CSS API.
    ///
    /// Try Robula+-style candidates, then a validated absolute positional path.
    /// Uniqueness and target identity refer to the normalized XPath DOM.
    ///
    /// # Errors
    ///
    /// Returns an error if the element is not attached to this document.
    pub fn generate_xpath_selector(&self, element: PyRef<'_, Element>) -> PyResult<Option<String>> {
        if self.closed {
            return Ok(None);
        }
        let source = element.source.ok_or_else(|| {
            PyValueError::new_err(
                "Selector generation requires an Element returned by this Document's CSS selection API",
            )
        })?;
        if source.document_id != self.document_id {
            return Err(PyValueError::new_err(
                "Element belongs to a different Document",
            ));
        }

        let Some(target_path) = generate_absolute_xpath_selector(self.dom.get_ref(), source.handle)
        else {
            return Ok(None);
        };
        let candidates =
            generate_robust_xpath_candidates(self.dom.get_ref(), source.handle).unwrap_or_default();
        let mut state_lock = self.ensure_xpath_state()?;
        let state = state_lock
            .as_mut()
            .ok_or_else(|| PyValueError::new_err("XPath state should be initialized"))?;
        find_unique_xpath_selector(
            &mut state.documents,
            state.document_handle,
            &target_path,
            candidates,
        )
    }

    /// Evaluate an `XPath` expression against the whole document.
    ///
    /// The expression must return element nodes; attribute/text results are not supported.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression is invalid or does not evaluate to element nodes.
    pub fn xpath(&self, expr: &str) -> PyResult<Vec<Element>> {
        if self.closed {
            return Ok(Vec::new());
        }
        let mut state_lock = self.ensure_xpath_state()?;
        let state = state_lock
            .as_mut()
            .ok_or_else(|| PyValueError::new_err("XPath state should be initialized"))?;
        evaluate_xpath_elements(&mut state.documents, state.document_handle, expr)
    }

    /// Return the first matching element for an `XPath` expression, or None.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression is invalid or does not evaluate to element nodes.
    pub fn xpath_first(&self, expr: &str) -> PyResult<Option<Element>> {
        if self.closed {
            return Ok(None);
        }
        let mut state_lock = self.ensure_xpath_state()?;
        let state = state_lock
            .as_mut()
            .ok_or_else(|| PyValueError::new_err("XPath state should be initialized"))?;
        evaluate_xpath_first_element(&mut state.documents, state.document_handle, expr)
    }

    /// Explicitly release parsed DOMs to free memory early.
    ///
    /// After calling, the document is reset to an empty state; selectors will
    /// return no results. Safe to call multiple times; it also runs when the
    /// Document is dropped.
    pub fn close(&mut self) {
        self.release_dom();
    }

    /// Support usage as a context manager to free resources on exit.
    fn __enter__(slf: PyRefMut<'_, Self>) -> PyRefMut<'_, Self> {
        slf
    }

    /// Support usage as a context manager to free resources on exit.
    fn __exit__(
        mut self_: PyRefMut<'_, Self>,
        _exc_type: Option<Bound<'_, PyAny>>,
        _exc_value: Option<Bound<'_, PyAny>>,
        _traceback: Option<Bound<'_, PyAny>>,
    ) {
        self_.close();
    }

    pub(crate) fn __repr__(&self) -> String {
        let len = self.dom.html().len();
        format!("<Document len_html={len}>")
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        self.release_dom();
    }
}
