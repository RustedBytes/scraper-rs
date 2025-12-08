use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::wrap_pyfunction;
use scraper::{Html, Selector};

/// Tiny helper to truncate text in __repr__.
fn truncate_for_repr(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

/// A single HTML element returned by a CSS selection.
///
/// This is a *snapshot* of an element: it stores tag, text, inner HTML
/// and attributes, all as owned data, so there are no lifetime issues
/// when used from Python.
#[pyclass(module = "scraper_rs")]
#[derive(Clone)]
pub struct Element {
    tag: String,
    text: String,
    inner_html: String,
    attrs: HashMap<String, String>,
}

#[pymethods]
impl Element {
    /// Tag name of the element (e.g. "div", "a").
    #[getter]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Normalized text content of the element.
    #[getter]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Inner HTML of the element (children only, not the outer tag).
    #[getter]
    pub fn html(&self) -> &str {
        &self.inner_html
    }

    /// Mapping of HTML attributes, e.g. {"href": "...", "class": "..."}.
    #[getter]
    pub fn attrs(&self) -> HashMap<String, String> {
        self.attrs.clone()
    }

    /// Return the value of a single attribute, or None if it doesn't exist.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs.get(name).cloned()
    }

    /// Convenience: behave like dict.get(key, default).
    pub fn get(&self, name: &str, default: Option<String>) -> Option<String> {
        self.attrs.get(name).cloned().or(default)
    }

    /// Convert this element to a plain dict.
    ///
    /// {
    ///   "tag": str,
    ///   "text": str,
    ///   "html": str,
    ///   "attrs": {str: str}
    /// }
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("tag", &self.tag)?;
        dict.set_item("text", &self.text)?;
        dict.set_item("html", &self.inner_html)?;
        dict.set_item("attrs", &self.attrs)?;
        Ok(dict.into())
    }

    /// Representation of the element for debugging.
    fn __repr__(&self) -> String {
        let text_preview = truncate_for_repr(self.text.trim(), 40);
        format!("<Element tag='{}' text={}>", self.tag, text_preview)
    }
}

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
    raw_html: String,
    html: Html,
}

#[pymethods]
impl Document {
    /// Create a Document from a raw HTML string.
    ///
    ///     doc = Document("<html>...</html>")
    #[new]
    pub fn new(html: &str) -> Self {
        Self {
            raw_html: html.to_string(),
            html: Html::parse_document(html),
        }
    }

    /// Alternate constructor: Document.from_html(html: str) -> Document
    #[staticmethod]
    pub fn from_html(html: &str) -> Self {
        Self::new(html)
    }

    /// Return the original HTML string.
    #[getter]
    pub fn html(&self) -> &str {
        &self.raw_html
    }

    /// All text content from the document, normalized and joined by spaces.
    #[getter]
    pub fn text(&self) -> String {
        self.html
            .root_element()
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Select all elements matching the given CSS selector.
    ///
    /// Returns a list[Element].
    ///
    ///     links = doc.select("a[href]")
    ///     for el in links:
    ///         print(el.text, el.attr("href"))
    pub fn select(&self, css: &str) -> PyResult<Vec<Element>> {
        let selector = Selector::parse(css)
            .map_err(|e| PyValueError::new_err(format!("Invalid CSS selector {css:?}: {e:?}")))?;

        let mut out = Vec::new();

        for el in self.html.select(&selector) {
            let tag = el.value().name().to_string();

            let text = el
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            let inner_html = el.inner_html();

            let mut attrs = HashMap::new();
            for (name, value) in el.value().attrs() {
                attrs.insert(name.to_string(), value.to_string());
            }

            out.push(Element {
                tag,
                text,
                inner_html,
                attrs,
            });
        }

        Ok(out)
    }

    /// Return the first matching element, or None if nothing matches.
    ///
    ///     first_link = doc.find("a[href]")
    ///     if first_link:
    ///         print(first_link.text)
    pub fn find(&self, css: &str) -> PyResult<Option<Element>> {
        let elements = self.select(css)?;
        Ok(elements.into_iter().next())
    }

    /// Shorthand for `select(css)`; more “requests-html” style.
    ///
    ///     doc.css("div.item")
    pub fn css(&self, css: &str) -> PyResult<Vec<Element>> {
        self.select(css)
    }

    fn __repr__(&self) -> String {
        let len = self.raw_html.len();
        format!("<Document len_html={}>", len)
    }
}

#[pyfunction]
fn parse(html: &str) -> Document {
    Document::from_html(html)
}

#[pyfunction]
fn select(html: &str, css: &str) -> PyResult<Vec<Element>> {
    let doc = Document::from_html(html);
    doc.select(css)
}

#[pyfunction]
fn first(html: &str, css: &str) -> PyResult<Option<Element>> {
    let doc = Document::from_html(html);
    doc.find(css)
}

/// Top-level module initializer.
#[pymodule]
fn scraper_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Classes
    m.add_class::<Document>()?;
    m.add_class::<Element>()?;

    // Top-level functions
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(select, m)?)?;
    m.add_function(wrap_pyfunction!(first, m)?)?;

    Ok(())
}
