use std::collections::HashMap;

use crate::tl_dom::{attrs_to_map, node_text, parse_owned_html_unlimited, tag_inner_html};

/// Tiny helper to truncate text in __repr__.
#[inline]
pub(crate) fn truncate_for_repr(s: &str, max_chars: usize) -> String {
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

#[inline]
fn push_normalized(out: &mut String, input: &str, needs_space: &mut bool) {
    for word in input.split_whitespace() {
        if *needs_space {
            out.push(' ');
        }
        out.push_str(word);
        *needs_space = true;
    }
}

#[inline]
pub(crate) fn normalize_text_nodes<'a, I>(chunks: I) -> String
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

#[inline]
pub(crate) fn inner_html_from_element_html(element_html: &str) -> String {
    parse_owned_html_unlimited(element_html.to_string())
        .ok()
        .and_then(|dom| {
            let dom = dom.get_ref();
            let parser = dom.parser();
            dom.children()
                .iter()
                .filter_map(|handle| handle.get(parser))
                .find_map(|node| node.as_tag())
                .map(|tag| tag_inner_html(tag, parser))
        })
        .unwrap_or_default()
}

#[inline]
pub(crate) fn text_from_element_html(element_html: &str) -> String {
    parse_owned_html_unlimited(element_html.to_string())
        .ok()
        .and_then(|dom| {
            let dom = dom.get_ref();
            let parser = dom.parser();
            dom.children()
                .iter()
                .filter_map(|handle| handle.get(parser))
                .find(|node| node.as_tag().is_some())
                .map(|node| node_text(node, parser))
        })
        .unwrap_or_default()
}

#[inline]
pub(crate) fn attrs_from_element_html(element_html: &str) -> HashMap<String, String> {
    parse_owned_html_unlimited(element_html.to_string())
        .ok()
        .and_then(|dom| {
            let dom = dom.get_ref();
            let parser = dom.parser();
            dom.children()
                .iter()
                .filter_map(|handle| handle.get(parser))
                .find_map(|node| node.as_tag())
                .map(attrs_to_map)
        })
        .unwrap_or_default()
}
