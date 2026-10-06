use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use html_escape::decode_html_entities;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use tl::queryselector::Selector;
use tl::{Node, NodeHandle, Parser, VDom};

use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::text::normalize_text_nodes;

pub(crate) type TlParser<'a> = Parser<'a, 32, 0, 0, 16, 16, 0>;
pub(crate) type TlVDom<'a> = VDom<'a, 32, 0, 0, 16, 16, 0>;

pub(crate) struct OwnedTlDom {
    dom: TlVDom<'static>,
    html: Box<str>,
}

impl OwnedTlDom {
    #[inline]
    pub(crate) fn parse(html: String) -> PyResult<Self> {
        let html = html.into_boxed_str();
        let html_ptr: *const str = &*html;
        // SAFETY: `html` is heap allocated and stored in the returned struct.
        // The DOM is dropped before `html` because fields are dropped in declaration order.
        let html_ref: &'static str = unsafe { &*html_ptr };
        let dom = tl::parse(html_ref, tl::ParserOptions::default())
            .map_err(|err| PyValueError::new_err(err.to_string()))?;

        Ok(Self { dom, html })
    }

    #[inline]
    pub(crate) fn get_ref(&self) -> &TlVDom<'static> {
        &self.dom
    }

    #[inline]
    pub(crate) fn html(&self) -> &str {
        &self.html
    }
}

#[inline]
pub(crate) fn parse_owned_html_with_raw(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<OwnedTlDom> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    parse_owned_html_unlimited(html_to_parse.into_owned())
}

#[inline]
pub(crate) fn parse_owned_html_unlimited(html: String) -> PyResult<OwnedTlDom> {
    OwnedTlDom::parse(html)
}

#[inline]
pub(crate) fn bytes_to_string(bytes: &tl::Bytes<'_>) -> String {
    bytes.as_utf8_str().into_owned()
}

/// Elements whose contents are raw text: character references inside them are
/// literal characters, not entities (HTML "raw text" and legacy equivalents).
const RAW_TEXT_ELEMENTS: [&str; 7] = [
    "script",
    "style",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "plaintext",
];

#[inline]
pub(crate) fn is_raw_text_element(tag: &tl::HTMLTag<'_>) -> bool {
    let name = tag.name().as_utf8_str();
    RAW_TEXT_ELEMENTS
        .iter()
        .any(|raw| raw.eq_ignore_ascii_case(name.as_ref()))
}

/// Decode character references the way an HTML parser does for text and attribute values.
///
/// Only complete references ending in `;` are decoded, so query strings such as
/// `?a=1&not=2` keep their literal `&` like browsers do in attribute values.
#[inline]
pub(crate) fn decode_entities(value: &str) -> Cow<'_, str> {
    decode_html_entities(value)
}

#[inline]
pub(crate) fn attrs_to_map(tag: &tl::HTMLTag<'_>) -> HashMap<String, String> {
    tag.attributes()
        .iter()
        .map(|(name, value)| {
            let value = value.unwrap_or_default();
            (name.into_owned(), decode_entities(&value).into_owned())
        })
        .collect()
}

/// Append the decoded text content of `node` (comments excluded) to `out`.
fn push_node_text(out: &mut String, node: &Node<'_>, parser: &TlParser<'_>, raw_text: bool) {
    match node {
        Node::Raw(bytes) => {
            let text = bytes.as_utf8_str();
            if raw_text {
                out.push_str(&text);
            } else {
                out.push_str(&decode_entities(&text));
            }
        }
        Node::Tag(tag) => {
            let raw_text = raw_text || is_raw_text_element(tag);
            for child in tag.children().top().iter() {
                if let Some(child) = child.get(parser) {
                    push_node_text(out, child, parser, raw_text);
                }
            }
        }
        Node::Comment(_) => {}
    }
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
    let mut text = String::new();
    push_node_text(&mut text, node, parser, false);
    normalize_text_nodes(std::iter::once(text.as_str()))
}

#[inline]
pub(crate) fn document_text(dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>) -> String {
    let parser = dom.parser();
    // Each root is normalized as its own chunk, matching the previous behaviour.
    let texts: Vec<String> = dom
        .children()
        .iter()
        .filter_map(|handle| handle.get(parser))
        .map(|node| {
            let mut text = String::new();
            push_node_text(&mut text, node, parser, false);
            text
        })
        .collect();
    normalize_text_nodes(texts.iter().map(String::as_str))
}

#[inline]
pub(crate) fn snapshot_node(node: &Node<'_>, parser: &TlParser<'_>) -> Option<Element> {
    let tag = node.as_tag()?;
    Some(Element::from_parts(
        bytes_to_string(tag.name()),
        node_outer_html(node, parser),
    ))
}

#[inline]
pub(crate) fn snapshot_handle_with_source(
    handle: NodeHandle,
    parser: &TlParser<'_>,
    document_id: u64,
) -> Option<Element> {
    let node = handle.get(parser)?;
    let tag = node.as_tag()?;
    Some(Element::from_dom_parts(
        bytes_to_string(tag.name()),
        node_outer_html(node, parser),
        document_id,
        handle,
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

pub(crate) fn select_handles_from_dom(
    dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>,
    css: &str,
) -> PyResult<Vec<NodeHandle>> {
    let selector = tl::parse_query_selector(css)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid CSS selector {css:?}")))?;
    let parser = dom.parser();
    let mut handles = Vec::new();
    let mut ancestors = Vec::new();

    for handle in dom.children() {
        collect_matching_handles(&mut handles, &selector, *handle, &mut ancestors, parser);
    }

    Ok(handles)
}

pub(crate) fn select_elements_from_dom(
    dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>,
    css: &str,
) -> PyResult<Vec<Element>> {
    let parser = dom.parser();
    Ok(select_handles_from_dom(dom, css)?
        .into_iter()
        .filter_map(|handle| handle.get(parser))
        .filter_map(|node| snapshot_node(node, parser))
        .collect())
}

pub(crate) fn select_elements_from_dom_with_source(
    dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>,
    css: &str,
    document_id: u64,
) -> PyResult<Vec<Element>> {
    let parser = dom.parser();
    Ok(select_handles_from_dom(dom, css)?
        .into_iter()
        .filter_map(|handle| snapshot_handle_with_source(handle, parser, document_id))
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

pub(crate) fn select_first_element_from_dom_with_source(
    dom: &tl::VDom<'_, 32, 0, 0, 16, 16, 0>,
    css: &str,
    document_id: u64,
) -> PyResult<Option<Element>> {
    let selector = tl::parse_query_selector(css)
        .ok_or_else(|| PyValueError::new_err(format!("Invalid CSS selector {css:?}")))?;
    let parser = dom.parser();
    let mut ancestors = Vec::new();

    for handle in dom.children() {
        if let Some(found) = find_matching_handle(&selector, *handle, &mut ancestors, parser) {
            return Ok(snapshot_handle_with_source(found, parser, document_id));
        }
    }

    Ok(None)
}

#[inline]
/// Convert forgiving HTML into markup that Xee's XML parser can consume.
///
/// The HTML parser removes the doctype and repairs nesting. This serializer then closes HTML
/// void elements, escapes raw-text content, and removes namespace declarations so ordinary HTML
/// XPath expressions continue to match unqualified element names.
pub(crate) fn normalized_document_html(html: &str) -> String {
    let Ok(dom) = parse_owned_html_unlimited(html.to_string()) else {
        return String::new();
    };
    let dom = dom.get_ref();
    let parser = dom.parser();
    let root_elements = dom
        .children()
        .iter()
        .filter_map(|handle| handle.get(parser))
        .filter(|node| node.as_tag().is_some())
        .count();

    let mut normalized = String::with_capacity(html.len());
    if root_elements == 1 {
        for handle in dom.children() {
            if let Some(tag) = handle.get(parser).and_then(|node| node.as_tag()) {
                serialize_xml_compatible_element(&mut normalized, tag, parser);
            }
        }
    } else {
        normalized.push_str("<xpath-document>");
        for handle in dom.children() {
            if let Some(node) = handle.get(parser) {
                serialize_xml_compatible_node(&mut normalized, node, parser);
            }
        }
        normalized.push_str("</xpath-document>");
    }

    normalized
}

pub(crate) fn xml_safe_name(name: &str) -> String {
    let mut safe = String::with_capacity(name.len().max(1));
    for (index, character) in name.chars().enumerate() {
        let valid = if index == 0 {
            character == '_' || character.is_alphabetic()
        } else {
            character == '_' || character == '-' || character == '.' || character.is_alphanumeric()
        };
        safe.push(if valid { character } else { '_' });
    }
    if safe.is_empty() {
        safe.push('_');
    }
    safe.to_lowercase()
}

fn push_xml_escaped(out: &mut String, value: &str, attribute: bool) {
    for character in decode_html_entities(value).chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            '\t'
            | '\n'
            | '\r'
            | '\u{20}'..='\u{d7ff}'
            | '\u{e000}'..='\u{fffd}'
            | '\u{10000}'..='\u{10ffff}' => out.push(character),
            _ => out.push('\u{fffd}'),
        }
    }
}

fn serialize_xml_compatible_node(out: &mut String, node: &Node<'_>, parser: &TlParser<'_>) {
    match node {
        Node::Tag(tag) => serialize_xml_compatible_element(out, tag, parser),
        Node::Raw(text) => push_xml_escaped(out, &bytes_to_string(text), false),
        // Comments do not affect element-selection results and malformed HTML comments are not
        // necessarily valid XML comments, so omit them from the XPath representation.
        Node::Comment(_) => {}
    }
}

fn serialize_xml_compatible_element(
    out: &mut String,
    tag: &tl::HTMLTag<'_>,
    parser: &TlParser<'_>,
) {
    let name = xml_safe_name(&bytes_to_string(tag.name()));
    out.push('<');
    out.push_str(&name);

    let mut emitted_attributes = HashSet::new();
    for (attribute_name, attribute_value) in tag.attributes().iter() {
        let attribute_name = xml_safe_name(attribute_name.as_ref());
        if attribute_name == "xmlns" || attribute_name.starts_with("xmlns_") {
            continue;
        }
        if !emitted_attributes.insert(attribute_name.clone()) {
            continue;
        }
        out.push(' ');
        out.push_str(&attribute_name);
        out.push_str("=\"");
        if let Some(attribute_value) = attribute_value {
            push_xml_escaped(out, attribute_value.as_ref(), true);
        }
        out.push('"');
    }

    out.push('>');
    for child in tag.children().top().iter() {
        if let Some(child) = child.get(parser) {
            serialize_xml_compatible_node(out, child, parser);
        }
    }
    out.push_str("</");
    out.push_str(&name);
    out.push('>');
}
