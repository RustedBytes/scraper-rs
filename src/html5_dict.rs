use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use tl::{Node, NodeHandle, Parser, VDomGuard};

use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};

const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";

#[derive(Clone, Copy)]
enum Html5ParseKind {
    Document,
    Fragment,
}

pub(crate) struct Html5DictTree {
    dom: VDomGuard,
    parse_kind: Html5ParseKind,
}

fn bytes_to_string(bytes: &tl::Bytes<'_>) -> String {
    bytes.as_utf8_str().into_owned()
}

fn attrs_to_map(tag: &tl::HTMLTag<'_>) -> HashMap<String, String> {
    tag.attributes()
        .iter()
        .map(|(name, value)| (name.into_owned(), value.unwrap_or_default().into_owned()))
        .collect()
}

fn comment_text(raw: &tl::Bytes<'_>) -> String {
    let text = raw.as_utf8_str();
    text.strip_prefix("<!--")
        .and_then(|text| text.strip_suffix("-->"))
        .unwrap_or(&text)
        .to_string()
}

fn tl_node_to_py(
    py: Python<'_>,
    parser: &Parser<'_, 32, 0, 0, 16, 16, 0>,
    node: &Node<'_>,
) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);

    match node {
        Node::Tag(tag) => {
            dict.set_item("node_type", "element")?;
            dict.set_item("tag", bytes_to_string(tag.name()))?;
            dict.set_item("namespace", HTML_NAMESPACE)?;
            dict.set_item("attrs", attrs_to_map(tag))?;

            let children = PyList::empty(py);
            for child in tag.children().top().iter() {
                if let Some(child_node) = child.get(parser) {
                    children.append(tl_node_to_py(py, parser, child_node)?)?;
                }
            }
            dict.set_item("children", children)?;
        }
        Node::Raw(text) => {
            dict.set_item("node_type", "text")?;
            dict.set_item("text", bytes_to_string(text))?;
            dict.set_item("children", PyList::empty(py))?;
        }
        Node::Comment(text) => {
            dict.set_item("node_type", "comment")?;
            dict.set_item("text", comment_text(text))?;
            dict.set_item("children", PyList::empty(py))?;
        }
    }

    Ok(dict.into())
}

fn append_top_level_children(
    py: Python<'_>,
    parser: &Parser<'_, 32, 0, 0, 16, 16, 0>,
    handles: &[NodeHandle],
) -> PyResult<Py<PyList>> {
    let children = PyList::empty(py);
    for handle in handles {
        if let Some(node) = handle.get(parser) {
            children.append(tl_node_to_py(py, parser, node)?)?;
        }
    }
    Ok(children.into())
}

fn parse_to_dict_tree(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
    parse_kind: Html5ParseKind,
) -> PyResult<Html5DictTree> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    // SAFETY: VDomGuard owns the input String and keeps it alive for the borrowed VDom.
    let dom = unsafe { tl::parse_owned(html_to_parse.into_owned(), tl::ParserOptions::default()) }
        .map_err(|err| PyValueError::new_err(err.to_string()))?;

    Ok(Html5DictTree { dom, parse_kind })
}

pub(crate) fn parse_document_to_dict_tree(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Html5DictTree> {
    parse_to_dict_tree(
        html,
        max_size_bytes,
        truncate_on_limit,
        Html5ParseKind::Document,
    )
}

pub(crate) fn parse_fragment_to_dict_tree(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Html5DictTree> {
    parse_to_dict_tree(
        html,
        max_size_bytes,
        truncate_on_limit,
        Html5ParseKind::Fragment,
    )
}

pub(crate) fn html5_tree_to_py_dict(py: Python<'_>, tree: Html5DictTree) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new(py);
    let node_type = match tree.parse_kind {
        Html5ParseKind::Document => "document",
        Html5ParseKind::Fragment => "document_fragment",
    };

    dict.set_item("node_type", node_type)?;
    let dom = tree.dom.get_ref();
    dict.set_item(
        "children",
        append_top_level_children(py, dom.parser(), dom.children())?,
    )?;
    dict.set_item("quirks_mode", "no-quirks")?;
    dict.set_item("errors", PyList::empty(py))?;

    Ok(dict.into())
}
