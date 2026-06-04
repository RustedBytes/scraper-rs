use std::cell::RefCell;
use std::rc::Rc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use xee_xpath::context::StaticContextBuilder;
use xee_xpath::query::SequenceQuery;
use xee_xpath::{DocumentHandle, Documents, Itemable, Queries, Query};

use crate::cache::FixedCache;
use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::tl_dom::normalized_document_html;

const XPATH_CACHE_CAPACITY: usize = 128;

thread_local! {
    static XPATH_CACHE: RefCell<FixedCache<Rc<SequenceQuery>>> =
        RefCell::new(FixedCache::new(XPATH_CACHE_CAPACITY));
}

pub(crate) fn parse_xpath_documents(
    html: &str,
    parse_target: &str,
) -> PyResult<(Documents, DocumentHandle)> {
    let mut documents = Documents::new();
    let document_handle = documents.add_string_without_uri(html).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse {parse_target} for XPath evaluation: {e}"
        ))
    })?;
    Ok((documents, document_handle))
}

pub(crate) fn compile_xpath(expr: &str) -> PyResult<Rc<SequenceQuery>> {
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

            Ok(Element::from_parts(tag, outer_html))
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

    Ok(Some(Element::from_parts(tag, outer_html)))
}

pub(crate) fn evaluate_xpath_elements(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<Vec<Element>> {
    let sequence = execute_xpath_sequence(documents, context_item, expr)?;
    evaluate_xpath_sequence_elements(&sequence, documents, expr)
}

pub(crate) fn evaluate_xpath_first_element(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<Option<Element>> {
    let sequence = execute_xpath_sequence(documents, context_item, expr)?;
    evaluate_xpath_sequence_first_element(&sequence, documents, expr)
}

pub(crate) fn xpath_with_limit(
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
            evaluate_fragment_xpath_with_fallback(html_to_parse.as_ref(), expr)
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn xpath_first_with_limit(
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
            evaluate_fragment_xpath_first_with_fallback(html_to_parse.as_ref(), expr)
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn evaluate_fragment_xpath(html: &str, expr: &str) -> PyResult<Vec<Element>> {
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

pub(crate) fn evaluate_fragment_xpath_first(html: &str, expr: &str) -> PyResult<Option<Element>> {
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

fn normalize_xpath_document_html(html: &str) -> String {
    normalized_document_html(html)
}

pub(crate) fn evaluate_fragment_xpath_with_fallback(
    html: &str,
    expr: &str,
) -> PyResult<Vec<Element>> {
    let normalized = normalize_xpath_document_html(html);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    match parse_xpath_documents(&normalized, "HTML document") {
        Ok((mut documents, document_handle)) => {
            evaluate_xpath_elements(&mut documents, document_handle, expr)
        }
        Err(_) => Ok(Vec::new()),
    }
}

pub(crate) fn evaluate_fragment_xpath_first_with_fallback(
    html: &str,
    expr: &str,
) -> PyResult<Option<Element>> {
    let normalized = normalize_xpath_document_html(html);
    if normalized.is_empty() {
        return Ok(None);
    }
    match parse_xpath_documents(&normalized, "HTML document") {
        Ok((mut documents, document_handle)) => {
            evaluate_xpath_first_element(&mut documents, document_handle, expr)
        }
        Err(_) => Ok(None),
    }
}

pub(crate) struct XPathDocumentState {
    pub(crate) documents: Documents,
    pub(crate) document_handle: DocumentHandle,
}
