use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque, hash_map::Entry};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::wrap_pyfunction;
use scraper::{Html, Selector, element_ref::ElementRef};
use sxd_xpath::{Context, Factory, Value, XPath, nodeset::Node as XPathNode};

const DEFAULT_MAX_PARSE_BYTES: usize = 1_073_741_824; // 1 GiB
const SELECTOR_CACHE_CAPACITY: usize = 256;
const XPATH_CACHE_CAPACITY: usize = 128;

struct FixedCache<T> {
    map: HashMap<String, T>,
    order: VecDeque<String>,
    capacity: usize,
}

impl<T> FixedCache<T> {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn get(&self, key: &str) -> Option<&T> {
        self.map.get(key)
    }

    fn insert(&mut self, key: String, value: T) {
        let key = match self.map.entry(key) {
            Entry::Occupied(mut entry) => {
                entry.insert(value);
                return;
            }
            Entry::Vacant(entry) => entry.into_key(),
        };

        if self.map.len() >= self.capacity
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }

        self.order.push_back(key.clone());
        self.map.insert(key, value);
    }
}

thread_local! {
    static SELECTOR_CACHE: RefCell<FixedCache<Arc<Selector>>> =
        RefCell::new(FixedCache::new(SELECTOR_CACHE_CAPACITY));
    static XPATH_CACHE: RefCell<FixedCache<Rc<XPath>>> =
        RefCell::new(FixedCache::new(XPATH_CACHE_CAPACITY));
}

fn effective_max_size(max_size_bytes: Option<usize>) -> usize {
    max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES)
}

fn ensure_within_size_limit<'a>(
    html: &'a str,
    max_size_bytes: usize,
    truncate_on_limit: bool,
) -> PyResult<Cow<'a, str>> {
    let len_bytes = html.len();
    if len_bytes > max_size_bytes {
        if truncate_on_limit {
            // Truncate to max_size_bytes, ensuring we don't split a UTF-8 character
            let mut truncate_at = max_size_bytes;
            while truncate_at > 0 && !html.is_char_boundary(truncate_at) {
                truncate_at -= 1;
            }
            return Ok(Cow::Owned(html[..truncate_at].to_string()));
        } else {
            return Err(PyValueError::new_err(format!(
                "HTML document is too large to parse: {len_bytes} bytes exceeds max_size_bytes={max_size_bytes}"
            )));
        }
    }

    Ok(Cow::Borrowed(html))
}

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

fn push_normalized(out: &mut String, input: &str, needs_space: &mut bool) {
    for word in input.split_whitespace() {
        if *needs_space {
            out.push(' ');
        }
        out.push_str(word);
        *needs_space = true;
    }
}

fn normalize_text_nodes<'a, I>(chunks: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let mut out = String::new();
    let mut needs_space = false;
    for chunk in chunks {
        push_normalized(&mut out, chunk, &mut needs_space);
    }
    out
}

fn normalize_xpath_text(node: XPathNode<'_>) -> String {
    let mut out = String::new();
    let mut needs_space = false;
    collect_xpath_text(node, &mut out, &mut needs_space);
    out
}

fn collect_xpath_text(node: XPathNode<'_>, out: &mut String, needs_space: &mut bool) {
    if let Some(text) = node.text() {
        push_normalized(out, text.text(), needs_space);
    }

    for child in node.children() {
        collect_xpath_text(child, out, needs_space);
    }
}

fn text_from_html_fragment(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    normalize_text_nodes(fragment.root_element().text())
}

fn attrs_from_element_html(element_html: &str) -> HashMap<String, String> {
    let fragment = Html::parse_fragment(element_html);
    let mut attrs = HashMap::new();

    for element_ref in fragment.root_element().children() {
        if let Some(element) = element_ref.value().as_element() {
            for (name, value) in element.attrs() {
                attrs.insert(name.to_string(), value.to_string());
            }
            break;
        }
    }

    attrs
}

fn attrs_from_element_ref(el: &ElementRef<'_>) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    for (name, value) in el.value().attrs() {
        attrs.insert(name.to_string(), value.to_string());
    }
    attrs
}

fn attrs_from_xpath_element(element: sxd_document::dom::Element<'_>) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    for attr in element.attributes().iter() {
        attrs.insert(
            attr.name().local_part().to_string(),
            attr.value().to_string(),
        );
    }
    attrs
}

/// A single HTML element returned by a CSS selection.
///
/// This is a *snapshot* of an element: it stores tag, text, inner HTML
/// and attributes, all as owned data, so there are no lifetime issues
/// when used from Python.
///
/// Properties are cached on first access (or eagerly for snapshots) for speed.
///
/// Note: This struct is NOT Clone because cached fields use OnceLock for
/// thread-safe interior mutability (required for async support).
/// If cloning is needed, use to_dict() and reconstruct.
#[pyclass(module = "scraper_rs")]
pub struct Element {
    tag: String,
    inner_html: String,
    // Cached fields stored in OnceLock for fast, thread-safe access.
    // Values are computed eagerly for CSS/XPath snapshots, or lazily from HTML as needed.
    text: OnceLock<String>,
    attrs: OnceLock<HashMap<String, String>>,
    // Store raw HTML element data for nested selections and lazy computation.
    // This avoids re-parsing when only specific properties are accessed.
    element_html: String,
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
            .get_or_init(|| text_from_html_fragment(&self.element_html))
            .clone()
    }

    /// Inner HTML of the element (children only, not the outer tag).
    #[getter]
    pub fn html(&self) -> &str {
        &self.inner_html
    }

    /// Mapping of HTML attributes, e.g. {"href": "...", "class": "..."}.
    #[getter]
    pub fn attrs(&self) -> HashMap<String, String> {
        self.attrs
            .get_or_init(|| attrs_from_element_html(&self.element_html))
            .clone()
    }

    /// Return the value of a single attribute, or None if it doesn't exist.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs
            .get_or_init(|| attrs_from_element_html(&self.element_html))
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
    pub fn select(&self, css: &str) -> PyResult<Vec<Element>> {
        select_fragment(&self.inner_html, css)
    }

    /// Return the first matching descendant element, or None if nothing matches.
    pub fn select_first(&self, css: &str) -> PyResult<Option<Element>> {
        select_fragment_first(&self.inner_html, css)
    }

    /// Return the first matching descendant element, or None if nothing matches.
    pub fn find(&self, css: &str) -> PyResult<Option<Element>> {
        self.select_first(css)
    }

    /// Alias for `select(css)`.
    pub fn css(&self, css: &str) -> PyResult<Vec<Element>> {
        self.select(css)
    }

    /// Evaluate an XPath expression against this element's children.
    ///
    /// The XPath runs inside this element; expressions must return element nodes.
    pub fn xpath(&self, expr: &str) -> PyResult<Vec<Element>> {
        evaluate_fragment_xpath(&self.inner_html, expr)
    }

    /// Return the first matching descendant for an XPath expression, or None.
    pub fn xpath_first(&self, expr: &str) -> PyResult<Option<Element>> {
        evaluate_fragment_xpath_first(&self.inner_html, expr)
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
        dict.set_item("text", self.text())?;
        dict.set_item("html", &self.inner_html)?;
        dict.set_item("attrs", self.attrs())?;
        Ok(dict.into())
    }

    /// Representation of the element for debugging.
    fn __repr__(&self) -> String {
        let text_str = self.text();
        let text_preview = truncate_for_repr(text_str.trim(), 40);
        format!("<Element tag='{}' text={}>", self.tag, text_preview)
    }
}

/// Convert a scraper ElementRef into our owned Element snapshot.
fn snapshot_element(el: ElementRef<'_>) -> Element {
    let tag = el.value().name().to_string();
    let inner_html = el.inner_html();
    let text = normalize_text_nodes(el.text());
    let attrs = attrs_from_element_ref(&el);

    // Store the full element HTML for lazy computation and nested selections
    let element_html = el.html();
    let text_cache = OnceLock::new();
    let _ = text_cache.set(text);
    let attrs_cache = OnceLock::new();
    let _ = attrs_cache.set(attrs);

    Element {
        tag,
        inner_html,
        element_html,
        text: text_cache,
        attrs: attrs_cache,
    }
}

fn parse_selector(css: &str) -> PyResult<Arc<Selector>> {
    SELECTOR_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(selector) = cache.get(css) {
            return Ok(selector.clone());
        }

        let selector =
            Arc::new(Selector::parse(css).map_err(|e| {
                PyValueError::new_err(format!("Invalid CSS selector {css:?}: {e:?}"))
            })?);
        cache.insert(css.to_string(), selector.clone());
        Ok(selector)
    })
}

fn select_fragment(html: &str, css: &str) -> PyResult<Vec<Element>> {
    let selector = parse_selector(css)?;
    let fragment = Html::parse_fragment(html);
    Ok(fragment
        .select(selector.as_ref())
        .map(snapshot_element)
        .collect())
}

fn select_fragment_first(html: &str, css: &str) -> PyResult<Option<Element>> {
    let selector = parse_selector(css)?;
    let fragment = Html::parse_fragment(html);
    Ok(fragment
        .select(selector.as_ref())
        .next()
        .map(snapshot_element))
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn serialize_node_into(buf: &mut String, node: XPathNode<'_>) {
    if let Some(element) = node.element() {
        let name = element.name().local_part();
        buf.push('<');
        buf.push_str(name);

        for attr in element.attributes().iter() {
            buf.push(' ');
            buf.push_str(attr.name().local_part());
            buf.push('=');
            buf.push('"');
            buf.push_str(&escape_html(attr.value()));
            buf.push('"');
        }

        buf.push('>');
        for child in element.children() {
            serialize_node_into(buf, child.into());
        }
        buf.push_str("</");
        buf.push_str(name);
        buf.push('>');
    } else if let Some(text) = node.text() {
        buf.push_str(&escape_html(text.text()));
    }
}

fn serialize_children(node: XPathNode<'_>) -> String {
    let mut buf = String::new();
    for child in node.children() {
        serialize_node_into(&mut buf, child);
    }
    buf
}

fn serialize_element(node: XPathNode<'_>) -> String {
    let mut buf = String::new();
    serialize_node_into(&mut buf, node);
    buf
}

fn snapshot_xpath_element(node: XPathNode<'_>) -> PyResult<Element> {
    let element = node.element().ok_or_else(|| {
        PyValueError::new_err("XPath expression must return element nodes for conversion")
    })?;

    let tag = element.name().local_part().to_string();
    let inner_html = serialize_children(node);
    let element_html = serialize_element(node);
    let text = normalize_xpath_text(node);
    let attrs = attrs_from_xpath_element(element);
    let text_cache = OnceLock::new();
    let _ = text_cache.set(text);
    let attrs_cache = OnceLock::new();
    let _ = attrs_cache.set(attrs);

    Ok(Element {
        tag,
        inner_html,
        element_html,
        text: text_cache,
        attrs: attrs_cache,
    })
}

fn compile_xpath(expr: &str) -> PyResult<Rc<XPath>> {
    XPATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(xpath) = cache.get(expr) {
            return Ok(xpath.clone());
        }

        let factory = Factory::new();
        let expression = factory
            .build(expr)
            .map_err(|e| PyValueError::new_err(format!("Invalid XPath {expr:?}: {e:?}")))?
            .ok_or_else(|| PyValueError::new_err("Provided XPath expression is empty"))?;
        let expression = Rc::new(expression);
        cache.insert(expr.to_string(), expression.clone());
        Ok(expression)
    })
}

fn evaluate_xpath_nodes<'d>(node: XPathNode<'d>, expr: &str) -> PyResult<Vec<XPathNode<'d>>> {
    let expression = compile_xpath(expr)?;
    let context = Context::new();
    let result = expression
        .evaluate(&context, node)
        .map_err(|e| PyValueError::new_err(format!("Failed to evaluate XPath {expr:?}: {e:?}")))?;

    match result {
        Value::Nodeset(nodeset) => Ok(nodeset.document_order().into_iter().collect()),
        other => Err(PyValueError::new_err(format!(
            "XPath {expr:?} must return a node set (got {other:?})"
        ))),
    }
}

fn evaluate_xpath_elements<'d>(node: XPathNode<'d>, expr: &str) -> PyResult<Vec<Element>> {
    evaluate_xpath_nodes(node, expr)?
        .into_iter()
        .map(snapshot_xpath_element)
        .collect()
}

fn evaluate_xpath_first_element<'d>(node: XPathNode<'d>, expr: &str) -> PyResult<Option<Element>> {
    let expression = compile_xpath(expr)?;
    let context = Context::new();
    let result = expression
        .evaluate(&context, node)
        .map_err(|e| PyValueError::new_err(format!("Failed to evaluate XPath {expr:?}: {e:?}")))?;

    match result {
        Value::Nodeset(nodeset) => nodeset
            .document_order()
            .into_iter()
            .next()
            .map(snapshot_xpath_element)
            .transpose(),
        other => Err(PyValueError::new_err(format!(
            "XPath {expr:?} must return a node set (got {other:?})"
        ))),
    }
}

fn find_first_element_by_name<'d>(node: XPathNode<'d>, name: &str) -> Option<XPathNode<'d>> {
    if let Some(element) = node.element()
        && element.name().local_part() == name
    {
        return Some(node);
    }

    for child in node.children() {
        if let Some(found) = find_first_element_by_name(child, name) {
            return Some(found);
        }
    }

    None
}

fn evaluate_fragment_xpath(html: &str, expr: &str) -> PyResult<Vec<Element>> {
    let mut wrapped = String::with_capacity(html.len() + "<xpath-fragment></xpath-fragment>".len());
    wrapped.push_str("<xpath-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</xpath-fragment>");
    let package = sxd_html::parse_html(&wrapped);
    let document = package.as_document();

    let Some(wrapper) = find_first_element_by_name(document.root().into(), "xpath-fragment") else {
        return Err(PyValueError::new_err(
            "Failed to parse HTML fragment for XPath evaluation",
        ));
    };

    evaluate_xpath_elements(wrapper, expr)
}

fn evaluate_fragment_xpath_first(html: &str, expr: &str) -> PyResult<Option<Element>> {
    let mut wrapped = String::with_capacity(html.len() + "<xpath-fragment></xpath-fragment>".len());
    wrapped.push_str("<xpath-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</xpath-fragment>");
    let package = sxd_html::parse_html(&wrapped);
    let document = package.as_document();

    let Some(wrapper) = find_first_element_by_name(document.root().into(), "xpath-fragment") else {
        return Err(PyValueError::new_err(
            "Failed to parse HTML fragment for XPath evaluation",
        ));
    };

    evaluate_xpath_first_element(wrapper, expr)
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
    xpath_package: Mutex<Option<sxd_document::Package>>,
    closed: bool,
}

impl Document {
    fn parse_with_limit(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        let max_size_bytes = effective_max_size(max_size_bytes);
        let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;

        // Only parse with html5ever (for CSS selectors)
        // XPath parsing will be done lazily when first needed
        let html_parsed = Html::parse_document(html_to_parse.as_ref());

        Ok(Self {
            raw_html: html_to_parse.into_owned(),
            html: html_parsed,
            xpath_package: Mutex::new(None),
            closed: false,
        })
    }

    /// Get or initialize the XPath package lazily.
    ///
    /// Panics if the mutex is poisoned (only happens if a panic occurred
    /// while holding the lock, which should not happen in normal operation).
    fn ensure_xpath_package(&self) -> std::sync::MutexGuard<'_, Option<sxd_document::Package>> {
        let mut package_lock = self
            .xpath_package
            .lock()
            .expect("XPath package mutex poisoned");

        // Check if already initialized
        if package_lock.is_none() {
            // Parse HTML for XPath support
            let package = sxd_html::parse_html(&self.raw_html);
            *package_lock = Some(package);
        }

        package_lock
    }

    /// Drop all DOM allocations and shrink owned strings.
    fn release_dom(&mut self) {
        if self.closed {
            return;
        }

        self.raw_html.clear();
        self.raw_html.shrink_to_fit();
        self.html = Html::parse_document("");
        // Mutex should never be poisoned here, but use expect for better error message
        *self
            .xpath_package
            .lock()
            .expect("XPath package mutex poisoned") = None;
        self.closed = true;
    }
}

#[pymethods]
impl Document {
    /// Create a Document from a raw HTML string.
    ///
    ///     doc = Document("<html>...</html>")
    #[new]
    #[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
    pub fn new(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        Self::parse_with_limit(html, max_size_bytes, truncate_on_limit)
    }

    /// Alternate constructor: Document.from_html(html: str) -> Document
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
        &self.raw_html
    }

    /// All text content from the document, normalized and joined by spaces.
    #[getter]
    pub fn text(&self) -> String {
        normalize_text_nodes(self.html.root_element().text())
    }

    /// Select all elements matching the given CSS selector.
    ///
    /// Returns a list[Element].
    ///
    ///     links = doc.select("a[href]")
    ///     for el in links:
    ///         print(el.text, el.attr("href"))
    pub fn select(&self, css: &str) -> PyResult<Vec<Element>> {
        let selector = parse_selector(css)?;
        Ok(self
            .html
            .select(selector.as_ref())
            .map(snapshot_element)
            .collect::<Vec<_>>())
    }

    /// Return the first matching element, or None if nothing matches.
    ///
    ///     first_link = doc.select_first("a[href]")
    pub fn select_first(&self, css: &str) -> PyResult<Option<Element>> {
        let selector = parse_selector(css)?;
        Ok(self
            .html
            .select(selector.as_ref())
            .next()
            .map(snapshot_element))
    }

    /// Return the first matching element, or None if nothing matches.
    ///
    ///     first_link = doc.find("a[href]")
    ///     if first_link:
    ///         print(first_link.text)
    pub fn find(&self, css: &str) -> PyResult<Option<Element>> {
        self.select_first(css)
    }

    /// Shorthand for `select(css)`; more “requests-html” style.
    ///
    ///     doc.css("div.item")
    pub fn css(&self, css: &str) -> PyResult<Vec<Element>> {
        self.select(css)
    }

    /// Evaluate an XPath expression against the whole document.
    ///
    /// The expression must return element nodes; attribute/text results are not supported.
    pub fn xpath(&self, expr: &str) -> PyResult<Vec<Element>> {
        let package_lock = self.ensure_xpath_package();
        // Safe to unwrap: ensure_xpath_package guarantees Some after returning
        let package = package_lock
            .as_ref()
            .expect("XPath package should be initialized");
        let document = package.as_document();
        evaluate_xpath_elements(document.root().into(), expr)
    }

    /// Return the first matching element for an XPath expression, or None.
    pub fn xpath_first(&self, expr: &str) -> PyResult<Option<Element>> {
        let package_lock = self.ensure_xpath_package();
        let package = package_lock
            .as_ref()
            .expect("XPath package should be initialized");
        let document = package.as_document();
        evaluate_xpath_first_element(document.root().into(), expr)
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
    ) -> PyResult<()> {
        self_.close();
        Ok(())
    }

    fn __repr__(&self) -> String {
        let len = self.raw_html.len();
        format!("<Document len_html={}>", len)
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        self.release_dom();
    }
}

#[pyfunction]
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
fn parse(html: &str, max_size_bytes: Option<usize>, truncate_on_limit: bool) -> PyResult<Document> {
    Document::from_html(html, max_size_bytes, truncate_on_limit)
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
fn select(
    py: Python<'_>,
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    py.detach(|| {
        let doc = Document::from_html(html, max_size_bytes, truncate_on_limit)?;
        doc.select(css)
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
fn select_first(
    py: Python<'_>,
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    py.detach(|| {
        let doc = Document::from_html(html, max_size_bytes, truncate_on_limit)?;
        doc.select_first(css)
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
fn first(
    py: Python<'_>,
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    py.detach(|| {
        let doc = Document::from_html(html, max_size_bytes, truncate_on_limit)?;
        doc.find(css)
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
fn xpath(
    py: Python<'_>,
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    py.detach(|| {
        let doc = Document::from_html(html, max_size_bytes, truncate_on_limit)?;
        doc.xpath(expr)
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
fn xpath_first(
    py: Python<'_>,
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    py.detach(|| {
        let doc = Document::from_html(html, max_size_bytes, truncate_on_limit)?;
        doc.xpath_first(expr)
    })
}

// Async versions using pyo3-async-runtimes

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
fn select_async(
    py: Python<'_>,
    html: String,
    css: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| {
                py.detach(|| {
                    let doc = Document::from_html(&html, max_size_bytes, truncate_on_limit)?;
                    doc.select(&css)
                })
            })
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
fn select_first_async(
    py: Python<'_>,
    html: String,
    css: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| {
                py.detach(|| {
                    let doc = Document::from_html(&html, max_size_bytes, truncate_on_limit)?;
                    doc.select_first(&css)
                })
            })
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, css, *, max_size_bytes=None, truncate_on_limit=false))]
fn first_async(
    py: Python<'_>,
    html: String,
    css: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| {
                py.detach(|| {
                    let doc = Document::from_html(&html, max_size_bytes, truncate_on_limit)?;
                    doc.find(&css)
                })
            })
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
fn xpath_async(
    py: Python<'_>,
    html: String,
    expr: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| {
                py.detach(|| {
                    let doc = Document::from_html(&html, max_size_bytes, truncate_on_limit)?;
                    doc.xpath(&expr)
                })
            })
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr, *, max_size_bytes=None, truncate_on_limit=false))]
fn xpath_first_async(
    py: Python<'_>,
    html: String,
    expr: String,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| {
                py.detach(|| {
                    let doc = Document::from_html(&html, max_size_bytes, truncate_on_limit)?;
                    doc.xpath_first(&expr)
                })
            })
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, css))]
fn _select_fragment_async(py: Python<'_>, html: String, css: String) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| py.detach(|| select_fragment(&html, &css)))
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, css))]
fn _select_first_fragment_async(
    py: Python<'_>,
    html: String,
    css: String,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| py.detach(|| select_fragment_first(&html, &css)))
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr))]
fn _xpath_fragment_async(py: Python<'_>, html: String, expr: String) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| py.detach(|| evaluate_fragment_xpath(&html, &expr)))
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

#[pyfunction]
#[pyo3(signature = (html, expr))]
fn _xpath_first_fragment_async(
    py: Python<'_>,
    html: String,
    expr: String,
) -> PyResult<Bound<'_, PyAny>> {
    let locals = pyo3_async_runtimes::TaskLocals::with_running_loop(py)?.copy_context(py)?;
    pyo3_async_runtimes::tokio::future_into_py_with_locals(py, locals, async move {
        tokio::task::spawn_blocking(move || {
            Python::attach(|py| py.detach(|| evaluate_fragment_xpath_first(&html, &expr)))
        })
        .await
        .map_err(|e| PyValueError::new_err(format!("Task join error: {e}")))?
    })
}

/// Top-level module initializer.
#[pymodule(gil_used = false)]
fn scraper_rs(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Classes
    m.add_class::<Document>()?;
    m.add_class::<Element>()?;

    // Top-level functions
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(select, m)?)?;
    m.add_function(wrap_pyfunction!(select_first, m)?)?;
    m.add_function(wrap_pyfunction!(first, m)?)?;
    m.add_function(wrap_pyfunction!(xpath, m)?)?;
    m.add_function(wrap_pyfunction!(xpath_first, m)?)?;

    // Async versions
    m.add_function(wrap_pyfunction!(select_async, m)?)?;
    m.add_function(wrap_pyfunction!(select_first_async, m)?)?;
    m.add_function(wrap_pyfunction!(first_async, m)?)?;
    m.add_function(wrap_pyfunction!(xpath_async, m)?)?;
    m.add_function(wrap_pyfunction!(xpath_first_async, m)?)?;
    m.add_function(wrap_pyfunction!(_select_fragment_async, m)?)?;
    m.add_function(wrap_pyfunction!(_select_first_fragment_async, m)?)?;
    m.add_function(wrap_pyfunction!(_xpath_fragment_async, m)?)?;
    m.add_function(wrap_pyfunction!(_xpath_first_fragment_async, m)?)?;

    // Package metadata
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
