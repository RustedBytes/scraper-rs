use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scraper::{Html, element_ref::ElementRef};

use crate::selectors::parse_selector;
use crate::text::normalize_text_nodes;

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

pub(crate) fn prettify_document_html(html: &str) -> String {
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

pub(crate) fn prettify_fragment_html(html: &str) -> PyResult<String> {
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
