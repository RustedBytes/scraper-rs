use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tl::queryselector::Selector;
use tl::{Node, NodeHandle, Parser, VDomGuard};

use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::text::normalize_text_nodes;

pub(crate) type TlParser<'a> = Parser<'a, 32, 0, 0, 16, 16, 0>;

#[inline]
pub(crate) fn parse_owned_html_with_raw(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<(String, VDomGuard)> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let raw_html = html_to_parse.into_owned();
    let dom = parse_owned_html_unlimited(raw_html.clone())?;
    Ok((raw_html, dom))
}

#[inline]
pub(crate) fn parse_owned_html_unlimited(html: String) -> PyResult<VDomGuard> {
    // SAFETY: VDomGuard owns the input String and keeps it alive for the borrowed VDom.
    unsafe { tl::parse_owned(html, tl::ParserOptions::default()) }
        .map_err(|err| PyValueError::new_err(err.to_string()))
}

#[inline]
pub(crate) fn bytes_to_string(bytes: &tl::Bytes<'_>) -> String {
    bytes.as_utf8_str().into_owned()
}

#[inline]
pub(crate) fn attrs_to_map(tag: &tl::HTMLTag<'_>) -> HashMap<String, String> {
    tag.attributes()
        .iter()
        .map(|(name, value)| (name.into_owned(), value.unwrap_or_default().into_owned()))
        .collect()
}

#[inline]
pub(crate) fn node_outer_html(node: &Node<'_>, parser: &TlParser<'_>) -> String {
    if let Some(tag) = node.as_tag() {
        return tag.raw().as_utf8_str().into_owned();
    }

    let mut out = String::new();
    let _ = node.write_outer_html(parser, &mut out);
    out
}

#[inline]
pub(crate) fn tag_inner_html(tag: &tl::HTMLTag<'_>, parser: &TlParser<'_>) -> String {
    tag.inner_html(parser)
}

#[inline]
pub(crate) fn node_text(node: &Node<'_>, parser: &TlParser<'_>) -> String {
    normalize_text_nodes(std::iter::once(node.inner_text(parser).as_ref()))
}

#[inline]
pub(crate) fn document_text(dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>) -> String {
    let parser = dom.parser();
    normalize_text_nodes(
        dom.children()
            .iter()
            .filter_map(|handle| handle.get(parser))
            .map(|node| node.inner_text(parser))
            .collect::<Vec<_>>()
            .iter()
            .map(|text| text.as_ref()),
    )
}

#[inline]
pub(crate) fn snapshot_node(node: &Node<'_>, parser: &TlParser<'_>) -> Option<Element> {
    let tag = node.as_tag()?;
    Some(Element::from_parts(
        bytes_to_string(tag.name()),
        node_outer_html(node, parser),
    ))
}

fn ancestor_matches(
    selector: &Selector<'_>,
    ancestors: &[NodeHandle],
    parser: &TlParser<'_>,
) -> bool {
    ancestors.iter().enumerate().any(|(idx, handle)| {
        handle.get(parser).is_some_and(|ancestor| {
            selector_matches_node(selector, ancestor, &ancestors[..idx], parser)
        })
    })
}

fn ancestor_matches_all(
    left: &Selector<'_>,
    right: &Selector<'_>,
    ancestors: &[NodeHandle],
    parser: &TlParser<'_>,
) -> bool {
    ancestors.iter().enumerate().any(|(idx, handle)| {
        handle.get(parser).is_some_and(|ancestor| {
            let ancestor_ancestors = &ancestors[..idx];
            selector_matches_node(left, ancestor, ancestor_ancestors, parser)
                && selector_matches_node(right, ancestor, ancestor_ancestors, parser)
        })
    })
}

fn parent_matches(
    selector: &Selector<'_>,
    ancestors: &[NodeHandle],
    parser: &TlParser<'_>,
) -> bool {
    ancestors
        .last()
        .and_then(|handle| handle.get(parser))
        .is_some_and(|parent| {
            let parent_ancestors = &ancestors[..ancestors.len().saturating_sub(1)];
            selector_matches_node(selector, parent, parent_ancestors, parser)
        })
}

fn parent_matches_all(
    left: &Selector<'_>,
    right: &Selector<'_>,
    ancestors: &[NodeHandle],
    parser: &TlParser<'_>,
) -> bool {
    ancestors
        .last()
        .and_then(|handle| handle.get(parser))
        .is_some_and(|parent| {
            let parent_ancestors = &ancestors[..ancestors.len().saturating_sub(1)];
            selector_matches_node(left, parent, parent_ancestors, parser)
                && selector_matches_node(right, parent, parent_ancestors, parser)
        })
}

fn selector_matches_node(
    selector: &Selector<'_>,
    node: &Node<'_>,
    ancestors: &[NodeHandle],
    parser: &TlParser<'_>,
) -> bool {
    if node.as_tag().is_none() {
        return false;
    }

    match selector {
        Selector::And(left, right) => {
            if let Selector::Descendant(right_left, right_right) = right.as_ref() {
                return selector_matches_node(right_right, node, ancestors, parser)
                    && ancestor_matches_all(left, right_left, ancestors, parser);
            }

            if let Selector::Parent(right_left, right_right) = right.as_ref() {
                return selector_matches_node(right_right, node, ancestors, parser)
                    && parent_matches_all(left, right_left, ancestors, parser);
            }

            selector_matches_node(left, node, ancestors, parser)
                && selector_matches_node(right, node, ancestors, parser)
        }
        Selector::Or(left, right) => {
            selector_matches_node(left, node, ancestors, parser)
                || selector_matches_node(right, node, ancestors, parser)
        }
        Selector::Descendant(left, right) => {
            selector_matches_node(right, node, ancestors, parser)
                && ancestor_matches(left, ancestors, parser)
        }
        Selector::Parent(left, right) => {
            selector_matches_node(right, node, ancestors, parser)
                && parent_matches(left, ancestors, parser)
        }
        _ => selector.matches(node),
    }
}

fn collect_matching_handles(
    out: &mut Vec<NodeHandle>,
    selector: &Selector<'_>,
    handle: NodeHandle,
    ancestors: &mut Vec<NodeHandle>,
    parser: &TlParser<'_>,
) {
    let Some(node) = handle.get(parser) else {
        return;
    };

    if selector_matches_node(selector, node, ancestors, parser) {
        out.push(handle);
    }

    if let Some(tag) = node.as_tag() {
        ancestors.push(handle);
        for child in tag.children().top().iter() {
            collect_matching_handles(out, selector, *child, ancestors, parser);
        }
        ancestors.pop();
    }
}

fn find_matching_handle(
    selector: &Selector<'_>,
    handle: NodeHandle,
    ancestors: &mut Vec<NodeHandle>,
    parser: &TlParser<'_>,
) -> Option<NodeHandle> {
    let node = handle.get(parser)?;

    if selector_matches_node(selector, node, ancestors, parser) {
        return Some(handle);
    }

    if let Some(tag) = node.as_tag() {
        ancestors.push(handle);
        for child in tag.children().top().iter() {
            if let Some(found) = find_matching_handle(selector, *child, ancestors, parser) {
                ancestors.pop();
                return Some(found);
            }
        }
        ancestors.pop();
    }

    None
}

pub(crate) fn select_elements_from_dom(
    dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>,
    css: &str,
) -> PyResult<Vec<Element>> {
    let selector = tl::parse_query_selector(css)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid CSS selector {css:?}")))?;
    let parser = dom.parser();
    let mut handles = Vec::new();
    let mut ancestors = Vec::new();

    for handle in dom.children() {
        collect_matching_handles(&mut handles, &selector, *handle, &mut ancestors, parser);
    }

    Ok(handles
        .into_iter()
        .filter_map(|handle| handle.get(parser))
        .filter_map(|node| snapshot_node(node, parser))
        .collect())
}

#[inline]
pub(crate) fn select_first_element_from_dom(
    dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>,
    css: &str,
) -> PyResult<Option<Element>> {
    let selector = tl::parse_query_selector(css)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid CSS selector {css:?}")))?;
    let parser = dom.parser();
    let mut ancestors = Vec::new();

    for handle in dom.children() {
        if let Some(found) = find_matching_handle(&selector, *handle, &mut ancestors, parser) {
            return Ok(found
                .get(parser)
                .and_then(|node| snapshot_node(node, parser)));
        }
    }

    Ok(None)
}

#[inline]
pub(crate) fn normalized_document_html(html: &str) -> String {
    parse_owned_html_unlimited(html.to_string())
        .map(|dom| dom.get_ref().outer_html())
        .unwrap_or_default()
}
