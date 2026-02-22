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
use xee_xpath::context::StaticContextBuilder;
use xee_xpath::query::SequenceQuery;
use xee_xpath::{DocumentHandle, Documents, Itemable, Queries, Query as XeeQuery};

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
    static XPATH_CACHE: RefCell<FixedCache<Rc<SequenceQuery>>> =
        RefCell::new(FixedCache::new(XPATH_CACHE_CAPACITY));
}

fn ensure_within_size_limit(
    html: &str,
    max_size_bytes: usize,
    truncate_on_limit: bool,
) -> PyResult<Cow<'_, str>> {
    let len_bytes = html.len();
    if len_bytes > max_size_bytes {
        if truncate_on_limit {
            // Truncate to max_size_bytes, ensuring we don't split a UTF-8 character
            let mut truncate_at = max_size_bytes;
            while truncate_at > 0 && !html.is_char_boundary(truncate_at) {
                truncate_at -= 1;
            }
            return Ok(Cow::Owned(html[..truncate_at].to_string()));
        }
        return Err(PyValueError::new_err(format!(
            "HTML document is too large to parse: {len_bytes} bytes exceeds max_size_bytes={max_size_bytes}"
        )));
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

fn inner_html_from_element_html(element_html: &str) -> String {
    let fragment = Html::parse_fragment(element_html);
    fragment
        .root_element()
        .children()
        .find_map(ElementRef::wrap)
        .map(|el| el.inner_html())
        .unwrap_or_default()
}

fn text_from_element_html(element_html: &str) -> String {
    let fragment = Html::parse_fragment(element_html);
    fragment
        .root_element()
        .children()
        .find_map(ElementRef::wrap)
        .map(|el| normalize_text_nodes(el.text()))
        .unwrap_or_default()
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
    tag: String,
    // Full serialized element HTML, kept as the only eagerly allocated HTML payload.
    outer_html: String,
    // Cached fields stored in OnceLock for fast, thread-safe access.
    // Values are computed lazily from outer_html on first access.
    inner_html: OnceLock<String>,
    text: OnceLock<String>,
    attrs: OnceLock<HashMap<String, String>>,
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
    fn __repr__(&self) -> String {
        let text_str = self.text();
        let text_preview = truncate_for_repr(text_str.trim(), 40);
        format!("<Element tag='{}' text={}>", self.tag, text_preview)
    }
}

/// Convert a scraper `ElementRef` into our owned Element snapshot.
fn snapshot_element(el: ElementRef<'_>) -> Element {
    let tag = el.value().name().to_string();
    // Store only the full element HTML; derive other fields lazily to reduce
    // per-element memory when callers only touch a subset of properties.
    let outer_html = el.html();

    Element {
        tag,
        outer_html,
        inner_html: OnceLock::new(),
        text: OnceLock::new(),
        attrs: OnceLock::new(),
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

fn push_indent(out: &mut String, level: usize, indent_size: usize) {
    let spaces = level.saturating_mul(indent_size);
    out.reserve(spaces);
    for _ in 0..spaces {
        out.push(' ');
    }
}

fn has_visible_text(text: &str) -> bool {
    text.split_whitespace().next().is_some()
}

fn normalized_text(text: &str) -> String {
    normalize_text_nodes(std::iter::once(text))
}

fn is_void_html_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

enum PrettyChild<'a> {
    Element(ElementRef<'a>),
    Text(String),
}

fn collect_pretty_children(element: ElementRef<'_>) -> Vec<PrettyChild<'_>> {
    let mut children = Vec::new();
    for child in element.children() {
        if let Some(child_element) = ElementRef::wrap(child) {
            children.push(PrettyChild::Element(child_element));
            continue;
        }

        if let Some(text) = child.value().as_text() {
            let normalized = normalized_text(text);
            if !normalized.is_empty() {
                children.push(PrettyChild::Text(normalized));
            }
        }
    }
    children
}

fn serialize_pretty_element_into(
    out: &mut String,
    element: ElementRef<'_>,
    level: usize,
    indent_size: usize,
) {
    let name = element.value().name();
    push_indent(out, level, indent_size);
    out.push('<');
    out.push_str(name);

    for (attr_name, attr_value) in element.value().attrs() {
        out.push(' ');
        out.push_str(attr_name);
        out.push('=');
        out.push('"');
        out.push_str(&escape_html(attr_value));
        out.push('"');
    }

    let children = collect_pretty_children(element);
    if children.is_empty() {
        if is_void_html_element(name) {
            out.push('>');
        } else {
            out.push_str("></");
            out.push_str(name);
            out.push('>');
        }
        out.push('\n');
        return;
    }

    let has_element_children = children
        .iter()
        .any(|child| matches!(child, PrettyChild::Element(_)));
    if !has_element_children
        && children.len() == 1
        && let PrettyChild::Text(text) = &children[0]
    {
        out.push('>');
        out.push_str(&escape_html(text));
        out.push_str("</");
        out.push_str(name);
        out.push_str(">\n");
        return;
    }

    out.push_str(">\n");

    for child in children {
        match child {
            PrettyChild::Element(child_element) => {
                serialize_pretty_element_into(out, child_element, level + 1, indent_size);
            }
            PrettyChild::Text(text) => {
                push_indent(out, level + 1, indent_size);
                out.push_str(&escape_html(&text));
                out.push('\n');
            }
        }
    }

    push_indent(out, level, indent_size);
    out.push_str("</");
    out.push_str(name);
    out.push_str(">\n");
}

fn prettify_document_html(html: &str) -> String {
    if !has_visible_text(html) {
        return String::new();
    }

    let parsed = Html::parse_document(html);
    let root = parsed.root_element();

    let mut out = String::new();
    serialize_pretty_element_into(&mut out, root, 0, 2);
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn prettify_fragment_html(html: &str) -> PyResult<String> {
    if !has_visible_text(html) {
        return Ok(String::new());
    }

    let mut wrapped =
        String::with_capacity(html.len() + "<prettify-fragment></prettify-fragment>".len());
    wrapped.push_str("<prettify-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</prettify-fragment>");

    let parsed = Html::parse_document(&wrapped);
    let selector = parse_selector("prettify-fragment")?;
    let Some(root_wrapper) = parsed.select(selector.as_ref()).next() else {
        return Err(PyValueError::new_err(
            "Failed to parse HTML fragment for prettify",
        ));
    };

    let mut out = String::new();
    for child in root_wrapper.children() {
        if let Some(child_element) = ElementRef::wrap(child) {
            serialize_pretty_element_into(&mut out, child_element, 0, 2);
            continue;
        }

        if let Some(text) = child.value().as_text() {
            let normalized = normalized_text(text);
            if normalized.is_empty() {
                continue;
            }
            out.push_str(&escape_html(&normalized));
            out.push('\n');
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

fn select_with_limit(
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let selector = parse_selector(css)?;
    let parsed = Html::parse_document(html_to_parse.as_ref());

    Ok(parsed
        .select(selector.as_ref())
        .map(snapshot_element)
        .collect::<Vec<_>>())
}

fn select_first_with_limit(
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let selector = parse_selector(css)?;
    let parsed = Html::parse_document(html_to_parse.as_ref());

    Ok(parsed
        .select(selector.as_ref())
        .next()
        .map(snapshot_element))
}

fn parse_xpath_documents(html: &str, parse_target: &str) -> PyResult<(Documents, DocumentHandle)> {
    let mut documents = Documents::new();
    let document_handle = documents.add_string_without_uri(html).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse {parse_target} for XPath evaluation: {e}"
        ))
    })?;
    Ok((documents, document_handle))
}

fn compile_xpath(expr: &str) -> PyResult<Rc<SequenceQuery>> {
    XPATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(query) = cache.get(expr) {
            return Ok(query.clone());
        }

        let queries = Queries::new(StaticContextBuilder::default());
        let query = queries
            .sequence(expr)
            .map_err(|e| PyValueError::new_err(format!("Invalid XPath {expr:?}: {e:?}")))?;
        let query = Rc::new(query);
        cache.insert(expr.to_string(), query.clone());
        Ok(query)
    })
}

fn execute_xpath_sequence(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<xee_xpath::Sequence> {
    let query = compile_xpath(expr)?;
    query
        .execute(documents, context_item)
        .map_err(|e| PyValueError::new_err(format!("Failed to evaluate XPath {expr:?}: {e:?}")))
}

fn evaluate_xpath_sequence_elements(
    sequence: &xee_xpath::Sequence,
    documents: &Documents,
    expr: &str,
) -> PyResult<Vec<Element>> {
    let xot = documents.xot();
    let element_nodes = sequence.elements(xot).map_err(|e| {
        PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
    })?;

    element_nodes
        .map(|node| {
            let node = node.map_err(|e| {
                PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
            })?;
            let element = xot.element(node).ok_or_else(|| {
                PyValueError::new_err("XPath expression must return element nodes for conversion")
            })?;
            let tag = xot.local_name_str(element.name()).to_string();
            let outer_html = xot.to_string(node).map_err(|e| {
                PyValueError::new_err(format!("Failed to serialize XPath element result: {e}"))
            })?;

            Ok(Element {
                tag,
                outer_html,
                inner_html: OnceLock::new(),
                text: OnceLock::new(),
                attrs: OnceLock::new(),
            })
        })
        .collect()
}

fn evaluate_xpath_sequence_first_element(
    sequence: &xee_xpath::Sequence,
    documents: &Documents,
    expr: &str,
) -> PyResult<Option<Element>> {
    let xot = documents.xot();
    let mut element_nodes = sequence.elements(xot).map_err(|e| {
        PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
    })?;

    let Some(node) = element_nodes.next() else {
        return Ok(None);
    };
    let node = node.map_err(|e| {
        PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
    })?;
    let element = xot.element(node).ok_or_else(|| {
        PyValueError::new_err("XPath expression must return element nodes for conversion")
    })?;
    let tag = xot.local_name_str(element.name()).to_string();
    let outer_html = xot.to_string(node).map_err(|e| {
        PyValueError::new_err(format!("Failed to serialize XPath element result: {e}"))
    })?;

    Ok(Some(Element {
        tag,
        outer_html,
        inner_html: OnceLock::new(),
        text: OnceLock::new(),
        attrs: OnceLock::new(),
    }))
}

fn evaluate_xpath_elements(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<Vec<Element>> {
    let sequence = execute_xpath_sequence(documents, context_item, expr)?;
    evaluate_xpath_sequence_elements(&sequence, documents, expr)
}

fn evaluate_xpath_first_element(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<Option<Element>> {
    let sequence = execute_xpath_sequence(documents, context_item, expr)?;
    evaluate_xpath_sequence_first_element(&sequence, documents, expr)
}

fn xpath_with_limit(
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    match parse_xpath_documents(html_to_parse.as_ref(), "HTML document") {
        Ok((mut documents, document_handle)) => {
            evaluate_xpath_elements(&mut documents, document_handle, expr)
        }
        Err(_) if truncate_on_limit => {
            if let Ok(elements) = evaluate_fragment_xpath(html_to_parse.as_ref(), expr) {
                return Ok(elements);
            }
            let fallback = html_to_parse
                .rfind('>')
                .map(|last_tag_end| &html_to_parse[..=last_tag_end])
                .unwrap_or("");
            if fallback.is_empty() {
                Ok(Vec::new())
            } else {
                evaluate_fragment_xpath(fallback, expr).or(Ok(Vec::new()))
            }
        }
        Err(err) => Err(err),
    }
}

fn xpath_first_with_limit(
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    match parse_xpath_documents(html_to_parse.as_ref(), "HTML document") {
        Ok((mut documents, document_handle)) => {
            evaluate_xpath_first_element(&mut documents, document_handle, expr)
        }
        Err(_) if truncate_on_limit => {
            if let Ok(element) = evaluate_fragment_xpath_first(html_to_parse.as_ref(), expr) {
                return Ok(element);
            }
            let fallback = html_to_parse
                .rfind('>')
                .map(|last_tag_end| &html_to_parse[..=last_tag_end])
                .unwrap_or("");
            if fallback.is_empty() {
                Ok(None)
            } else {
                evaluate_fragment_xpath_first(fallback, expr).or(Ok(None))
            }
        }
        Err(err) => Err(err),
    }
}

fn evaluate_fragment_xpath(html: &str, expr: &str) -> PyResult<Vec<Element>> {
    let mut wrapped = String::with_capacity(html.len() + "<xpath-fragment></xpath-fragment>".len());
    wrapped.push_str("<xpath-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</xpath-fragment>");
    let (mut documents, document_handle) = parse_xpath_documents(&wrapped, "HTML fragment")?;
    let root = documents.document_node(document_handle).ok_or_else(|| {
        PyValueError::new_err("Failed to parse HTML fragment for XPath evaluation")
    })?;
    let root_element = documents.xot().document_element(root).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse HTML fragment for XPath evaluation: {e}"
        ))
    })?;

    evaluate_xpath_elements(&mut documents, root_element, expr)
}

fn evaluate_fragment_xpath_first(html: &str, expr: &str) -> PyResult<Option<Element>> {
    let mut wrapped = String::with_capacity(html.len() + "<xpath-fragment></xpath-fragment>".len());
    wrapped.push_str("<xpath-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</xpath-fragment>");
    let (mut documents, document_handle) = parse_xpath_documents(&wrapped, "HTML fragment")?;
    let root = documents.document_node(document_handle).ok_or_else(|| {
        PyValueError::new_err("Failed to parse HTML fragment for XPath evaluation")
    })?;
    let root_element = documents.xot().document_element(root).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse HTML fragment for XPath evaluation: {e}"
        ))
    })?;

    evaluate_xpath_first_element(&mut documents, root_element, expr)
}

struct XPathDocumentState {
    documents: Documents,
    document_handle: DocumentHandle,
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
    xpath_state: Mutex<Option<XPathDocumentState>>,
    closed: bool,
}

impl Document {
    fn parse_with_limit(
        html: &str,
        max_size_bytes: Option<usize>,
        truncate_on_limit: bool,
    ) -> PyResult<Self> {
        let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
        let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;

        // Only parse with html5ever (for CSS selectors)
        // XPath parsing will be done lazily when first needed
        let html_parsed = Html::parse_document(html_to_parse.as_ref());

        Ok(Self {
            raw_html: html_to_parse.into_owned(),
            html: html_parsed,
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
                parse_xpath_documents(&self.raw_html, "HTML document")?;
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

        self.raw_html.clear();
        self.raw_html.shrink_to_fit();
        self.html = Html::parse_document("");
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
        &self.raw_html
    }

    /// All text content from the document, normalized and joined by spaces.
    #[getter]
    pub fn text(&self) -> String {
        normalize_text_nodes(self.html.root_element().text())
    }

    /// Return the current document HTML formatted with indentation.
    ///
    /// # Errors
    ///
    /// This currently does not return errors, but the `PyResult` is preserved for API consistency.
    pub fn prettify(&self) -> PyResult<String> {
        Ok(prettify_document_html(&self.raw_html))
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
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
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

    fn __repr__(&self) -> String {
        let len = self.raw_html.len();
        format!("<Document len_html={len}>")
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
#[pyo3(signature = (html, *, max_size_bytes=None, truncate_on_limit=false))]
fn prettify(
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
fn select(
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
fn select_first(
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
fn first(
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
fn xpath(
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
fn xpath_first(
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
                py.detach(|| select_with_limit(&html, &css, max_size_bytes, truncate_on_limit))
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
                    select_first_with_limit(&html, &css, max_size_bytes, truncate_on_limit)
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
                    select_first_with_limit(&html, &css, max_size_bytes, truncate_on_limit)
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
                py.detach(|| xpath_with_limit(&html, &expr, max_size_bytes, truncate_on_limit))
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
                    xpath_first_with_limit(&html, &expr, max_size_bytes, truncate_on_limit)
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
    m.add_function(wrap_pyfunction!(prettify, m)?)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    const SAMPLE_HTML: &str = r#"
        <html>
          <body>
            <div class="item" data-id="1"><a href="/a">First</a></div>
            <div class="item" data-id="2"><a href="/b">Second</a></div>
          </body>
        </html>
    "#;

    fn init_python() {
        static INIT: Once = Once::new();
        INIT.call_once(Python::initialize);
    }

    #[test]
    fn fixed_cache_evicts_oldest_entry() {
        let mut cache = FixedCache::new(2);
        cache.insert("first".to_string(), 1_u8);
        cache.insert("second".to_string(), 2_u8);
        cache.insert("third".to_string(), 3_u8);

        assert!(cache.get("first").is_none());
        assert_eq!(cache.get("second"), Some(&2_u8));
        assert_eq!(cache.get("third"), Some(&3_u8));
    }

    #[test]
    fn ensure_within_size_limit_returns_borrowed_when_within_limit() {
        let html = "<div>ok</div>";
        let limited = ensure_within_size_limit(html, html.len(), false).unwrap();
        assert!(matches!(limited, Cow::Borrowed(_)));
    }

    #[test]
    fn ensure_within_size_limit_truncates_on_utf8_boundary() {
        let html = "<p>ab😀cd</p>";
        let emoji_start = html.find('😀').unwrap();
        let limit_inside_emoji = emoji_start + 1;

        let truncated = ensure_within_size_limit(html, limit_inside_emoji, true)
            .unwrap()
            .into_owned();

        assert!(truncated.len() < limit_inside_emoji);
        assert!(truncated.is_char_boundary(truncated.len()));
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn ensure_within_size_limit_errors_without_truncate() {
        let html = "<div>too big</div>";
        assert!(ensure_within_size_limit(html, 4, false).is_err());
    }

    #[test]
    fn normalize_text_nodes_collapses_whitespace() {
        let normalized = normalize_text_nodes([" one\t", "\n two  ", "three "]);
        assert_eq!(normalized, "one two three");
    }

    #[test]
    fn extractors_return_expected_element_parts() {
        let element_html = r#"<div id="x" class="item"><span> Hello </span><b>World</b></div>"#;

        assert_eq!(
            inner_html_from_element_html(element_html),
            "<span> Hello </span><b>World</b>"
        );
        assert_eq!(text_from_element_html(element_html), "Hello World");

        let attrs = attrs_from_element_html(element_html);
        assert_eq!(attrs.get("id"), Some(&"x".to_string()));
        assert_eq!(attrs.get("class"), Some(&"item".to_string()));
    }

    #[test]
    fn parse_selector_reuses_cached_instances() {
        let first = parse_selector("div.item").unwrap();
        let second = parse_selector("div.item").unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn compile_xpath_reuses_cached_instances() {
        init_python();
        let first = compile_xpath(".//div").unwrap();
        let second = compile_xpath(".//div").unwrap();
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn select_fragment_helpers_return_expected_matches() {
        let html = r#"<section><a href="/a">A</a><a href="/b">B</a></section>"#;

        let all = select_fragment(html, "a[href]").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].text(), "A");
        assert_eq!(all[1].attr("href"), Some("/b".to_string()));

        let first = select_fragment_first(html, "a[href]").unwrap().unwrap();
        assert_eq!(first.text(), "A");
    }

    #[test]
    fn fragment_xpath_helpers_return_expected_matches() {
        init_python();
        let html = r#"<ul><li><a>A</a></li><li><a>B</a></li></ul>"#;

        let all = evaluate_fragment_xpath(html, ".//li/a").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].text(), "A");
        assert_eq!(all[1].text(), "B");

        let missing = evaluate_fragment_xpath_first(html, ".//p").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn select_and_xpath_with_limit_respect_truncation() {
        init_python();
        let html = concat!(
            r#"<div id="start">begin</div>"#,
            r#"<div id="middle">this content is intentionally long for truncation</div>"#,
            r#"<div id="end">finish</div>"#
        );
        let limit = html.find("id=\"end\"").unwrap();

        let start_css = select_with_limit(html, "#start", Some(limit), true).unwrap();
        let end_css = select_with_limit(html, "#end", Some(limit), true).unwrap();
        assert_eq!(start_css.len(), 1);
        assert!(end_css.is_empty());

        let start_xpath = xpath_with_limit(html, "//*[@id='start']", Some(limit), true);
        assert!(start_xpath.is_ok(), "xpath should succeed for retained prefix");
        assert_eq!(start_xpath.unwrap_or_default().len(), 1);

        let end_xpath = xpath_with_limit(html, "//*[@id='end']", Some(limit), true);
        assert!(end_xpath.is_ok(), "xpath should succeed for truncated suffix");
        assert!(end_xpath.unwrap_or_default().is_empty());
    }

    #[test]
    fn prettify_helpers_format_document_and_fragment() {
        let doc_pretty =
            prettify_document_html(r#"<div id="x"><span>Hi</span><p>A &amp; B</p><br></div>"#);
        assert!(doc_pretty.contains(r#"<div id="x">"#));
        assert!(doc_pretty.contains("<span>Hi</span>"));
        assert!(doc_pretty.contains("<p>A &amp; B</p>"));
        assert!(doc_pretty.contains("<br>"));

        let fragment_pretty =
            prettify_fragment_html(r#"hello <span data-x="1"> world </span>"#).unwrap();
        assert_eq!(fragment_pretty, "hello\n<span data-x=\"1\">world</span>");
    }

    #[test]
    fn document_close_clears_state_and_is_idempotent() {
        init_python();
        let mut doc = Document::new(SAMPLE_HTML, None, false).unwrap();

        assert_eq!(doc.select("a").unwrap().len(), 2);
        let initial_xpath = doc.xpath(".//a");
        assert!(initial_xpath.is_ok(), "xpath should succeed before close");
        assert_eq!(initial_xpath.unwrap_or_default().len(), 2);

        doc.close();
        doc.close();

        assert_eq!(doc.html(), "");
        assert_eq!(doc.text(), "");
        assert!(doc.select("a").unwrap().is_empty());
        assert!(doc.select_first("a").unwrap().is_none());
        let closed_xpath = doc.xpath(".//a");
        assert!(closed_xpath.is_ok(), "xpath should succeed after close");
        assert!(closed_xpath.unwrap_or_default().is_empty());
        let closed_xpath_first = doc.xpath_first(".//a");
        assert!(
            closed_xpath_first.is_ok(),
            "xpath_first should succeed after close"
        );
        assert!(closed_xpath_first.unwrap_or(None).is_none());
        assert_eq!(doc.prettify().unwrap(), "");
    }
}
