use std::collections::HashMap;
use std::sync::OnceLock;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::prettify::prettify_fragment_html;
use crate::selectors::{select_fragment, select_fragment_first};
use crate::text::{
    attrs_from_element_html, inner_html_from_element_html, text_from_element_html,
    truncate_for_repr,
};
use crate::xpath::{evaluate_fragment_xpath, evaluate_fragment_xpath_first};

/// A single HTML element returned by a CSS selection.
///
/// This is a *snapshot* of an element: it stores tag and serialized outer HTML
/// eagerly, and computes text/inner HTML/attributes lazily as owned data.
/// This keeps Python usage lifetime-safe while reducing upfront allocations.
///
/// Properties are cached on first access for speed and reduced memory pressure.
///
/// Note: This struct is NOT Clone because cached fields use `OnceLock` for
/// thread-safe interior mutability (required for async support).
/// If cloning is needed, use `to_dict()` and reconstruct.
#[pyclass(module = "scraper_rs")]
pub struct Element {
    pub(crate) tag: String,
    // Full serialized element HTML, kept as the only eagerly allocated HTML payload.
    pub(crate) outer_html: String,
    // Cached fields stored in OnceLock for fast, thread-safe access.
    // Values are computed lazily from outer_html on first access.
    pub(crate) inner_html: OnceLock<String>,
    pub(crate) text: OnceLock<String>,
    pub(crate) attrs: OnceLock<HashMap<String, String>>,
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
    pub fn text(&self) -> String {
        self.text
            .get_or_init(|| text_from_element_html(&self.outer_html))
            .clone()
    }

    /// Inner HTML of the element (children only, not the outer tag).
    #[getter]
    pub fn html(&self) -> &str {
        self.inner_html
            .get_or_init(|| inner_html_from_element_html(&self.outer_html))
    }

    /// Mapping of HTML attributes, e.g. {"href": "...", "class": "..."}.
    #[getter]
    pub fn attrs(&self) -> HashMap<String, String> {
        self.attrs
            .get_or_init(|| attrs_from_element_html(&self.outer_html))
            .clone()
    }

    /// Return the value of a single attribute, or None if it doesn't exist.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .get_or_init(|| attrs_from_element_html(&self.outer_html))
            .get(name)
            .cloned()
    }

    /// Convenience: behave like dict.get(key, default).
    pub fn get(&self, name: &str, default: Option<String>) -> Option<String> {
        self.attr(name).or(default)
    }

    /// Select elements inside this element's inner HTML using a CSS selector.
    ///
    ///     item = doc.find(".item")
    ///     links = item.select("a[href]")
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn select(&self, css: &str) -> PyResult<Vec<Element>> {
        select_fragment(self.html(), css)
    }

    /// Return the first matching descendant element, or None if nothing matches.
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn select_first(&self, css: &str) -> PyResult<Option<Element>> {
        select_fragment_first(self.html(), css)
    }

    /// Return the first matching descendant element, or None if nothing matches.
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn find(&self, css: &str) -> PyResult<Option<Element>> {
        self.select_first(css)
    }

    /// Alias for `select(css)`.
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub fn css(&self, css: &str) -> PyResult<Vec<Element>> {
        self.select(css)
    }

    /// Evaluate an `XPath` expression against this element's children.
    ///
    /// The `XPath` runs inside this element; expressions must return element nodes.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression is invalid or does not evaluate to element nodes.
    pub fn xpath(&self, expr: &str) -> PyResult<Vec<Element>> {
        evaluate_fragment_xpath(self.html(), expr)
    }

    /// Return the first matching descendant for an `XPath` expression, or None.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression is invalid or does not evaluate to element nodes.
    pub fn xpath_first(&self, expr: &str) -> PyResult<Option<Element>> {
        evaluate_fragment_xpath_first(self.html(), expr)
    }

    /// Return this element's outer HTML formatted with indentation.
    ///
    /// # Errors
    ///
    /// This currently does not return errors, but the `PyResult` is preserved for API consistency.
    pub fn prettify(&self) -> PyResult<String> {
        prettify_fragment_html(&self.outer_html)
    }

    /// Convert this element to a plain dict.
    ///
    /// {
    ///   "tag": str,
    ///   "text": str,
    ///   "html": str,
    ///   "attrs": {str: str}
    /// }
    ///
    /// # Errors
    ///
    /// Returns an error if Python dictionary construction fails.
    pub fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("tag", &self.tag)?;
        dict.set_item("text", self.text())?;
        dict.set_item("html", self.html())?;
        dict.set_item("attrs", self.attrs())?;
        Ok(dict.into())
    }

    /// Representation of the element for debugging.
    pub(crate) fn __repr__(&self) -> String {
        let text_str = self.text();
        let text_preview = truncate_for_repr(text_str.trim(), 40);
        format!("<Element tag='{}' text={}>", self.tag, text_preview)
    }
}

impl Element {
    #[inline]
    pub(crate) fn from_parts(tag: String, outer_html: String) -> Self {
        Self {
            tag,
            outer_html,
            inner_html: OnceLock::new(),
            text: OnceLock::new(),
            attrs: OnceLock::new(),
        }
    }
}
