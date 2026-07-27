use pyo3::prelude::*;

use crate::text::normalize_text_nodes;
use crate::tl_dom::{TlParser, bytes_to_string, parse_owned_html_unlimited};

#[inline]
pub(crate) fn escape_html(value: &str) -> String {
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

#[inline]
fn entity_reference_len(value: &str) -> Option<usize> {
    let semicolon = value.find(';')?;
    let reference = &value[..semicolon];
    let valid = if let Some(numeric) = reference.strip_prefix('#') {
        if let Some(hex) = numeric
            .strip_prefix('x')
            .or_else(|| numeric.strip_prefix('X'))
        {
            !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        } else {
            !numeric.is_empty() && numeric.bytes().all(|byte| byte.is_ascii_digit())
        }
    } else {
        !reference.is_empty() && reference.bytes().all(|byte| byte.is_ascii_alphanumeric())
    };
    valid.then_some(semicolon + 1)
}

#[inline]
fn escape_html_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    let mut rest = value;
    while let Some((before, after_ampersand)) = rest.split_once('&') {
        escaped.push_str(&escape_html(before));
        rest = &rest[before.len() + 1..];
        if let Some(reference_len) = entity_reference_len(after_ampersand) {
            escaped.push('&');
            escaped.push_str(&after_ampersand[..reference_len]);
            rest = &after_ampersand[reference_len..];
        } else {
            escaped.push_str("&amp;");
        }
    }
    escaped.push_str(&escape_html(rest));
    escaped
}

#[inline]
fn push_indent(out: &mut String, level: usize, indent_size: usize) {
    let spaces = level.saturating_mul(indent_size);
    out.reserve(spaces);
    for _ in 0..spaces {
        out.push(' ');
    }
}

#[inline]
fn has_visible_text(text: &str) -> bool {
    text.split_whitespace().next().is_some()
}

#[inline]
fn normalized_text(text: &str) -> String {
    normalize_text_nodes(std::iter::once(text))
}

#[inline]
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

enum PrettyChild {
    Element(tl::NodeHandle),
    Text(String),
}

fn collect_pretty_children(element: &tl::HTMLTag<'_>, parser: &TlParser<'_>) -> Vec<PrettyChild> {
    let mut children = Vec::new();
    for child in element.children().top().iter() {
        if let Some(child_node) = child.get(parser) {
            match child_node {
                tl::Node::Tag(_) => children.push(PrettyChild::Element(*child)),
                tl::Node::Raw(text) => {
                    let normalized = normalized_text(&bytes_to_string(text));
                    if !normalized.is_empty() {
                        children.push(PrettyChild::Text(normalized));
                    }
                }
                tl::Node::Comment(_) => {}
            }
        }
    }
    children
}

#[inline]
fn raw_attr_suffix(element: &tl::HTMLTag<'_>) -> String {
    let raw = element.raw().as_utf8_str();
    let Some(open_end) = raw.find('>') else {
        return String::new();
    };
    let open = &raw[..open_end];
    let Some(after_name) = open.find(char::is_whitespace) else {
        return String::new();
    };
    let attrs = open[after_name..].trim_end_matches('/').trim_end();
    if attrs.is_empty() {
        String::new()
    } else {
        attrs.to_string()
    }
}

fn serialize_pretty_element_into(
    out: &mut String,
    element: &tl::HTMLTag<'_>,
    parser: &TlParser<'_>,
    level: usize,
    indent_size: usize,
) {
    let name = bytes_to_string(element.name());
    push_indent(out, level, indent_size);
    out.push('<');
    out.push_str(&name);
    out.push_str(&raw_attr_suffix(element));

    let children = collect_pretty_children(element, parser);
    if children.is_empty() {
        if is_void_html_element(&name) {
            out.push('>');
        } else {
            out.push_str("></");
            out.push_str(&name);
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
        out.push_str(&escape_html_text(text));
        out.push_str("</");
        out.push_str(&name);
        out.push_str(">\n");
        return;
    }

    out.push_str(">\n");

    for child in children {
        match child {
            PrettyChild::Element(child_handle) => {
                if let Some(child_element) = child_handle.get(parser).and_then(|node| node.as_tag())
                {
                    serialize_pretty_element_into(
                        out,
                        child_element,
                        parser,
                        level + 1,
                        indent_size,
                    );
                }
            }
            PrettyChild::Text(text) => {
                push_indent(out, level + 1, indent_size);
                out.push_str(&escape_html_text(&text));
                out.push('\n');
            }
        }
    }

    push_indent(out, level, indent_size);
    out.push_str("</");
    out.push_str(&name);
    out.push_str(">\n");
}

pub(crate) fn prettify_document_html(html: &str) -> String {
    if !has_visible_text(html) {
        return String::new();
    }

    let Ok(parsed) = parse_owned_html_unlimited(html.to_string()) else {
        return String::new();
    };
    let dom = parsed.get_ref();
    let parser = dom.parser();

    let mut out = String::new();
    for handle in dom.children() {
        if let Some(node) = handle.get(parser) {
            match node {
                tl::Node::Tag(tag) => serialize_pretty_element_into(&mut out, tag, parser, 0, 2),
                tl::Node::Raw(text) => {
                    let normalized = normalized_text(&bytes_to_string(text));
                    if !normalized.is_empty() {
                        out.push_str(&escape_html_text(&normalized));
                        out.push('\n');
                    }
                }
                tl::Node::Comment(_) => {}
            }
        }
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

pub(crate) fn prettify_fragment_html(html: &str) -> PyResult<String> {
    if !has_visible_text(html) {
        return Ok(String::new());
    }

    let parsed = parse_owned_html_unlimited(html.to_string())?;
    let dom = parsed.get_ref();
    let parser = dom.parser();

    let mut out = String::new();
    for child in dom.children() {
        if let Some(child_node) = child.get(parser) {
            match child_node {
                tl::Node::Tag(child_element) => {
                    serialize_pretty_element_into(&mut out, child_element, parser, 0, 2);
                }
                tl::Node::Raw(text) => {
                    let normalized = normalized_text(&bytes_to_string(text));
                    if normalized.is_empty() {
                        continue;
                    }
                    out.push_str(&escape_html_text(&normalized));
                    out.push('\n');
                }
                tl::Node::Comment(_) => {}
            }
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}
