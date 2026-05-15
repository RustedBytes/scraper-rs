use std::collections::HashMap;

use scraper::{Html, element_ref::ElementRef};

/// Tiny helper to truncate text in __repr__.
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

fn push_normalized(out: &mut String, input: &str, needs_space: &mut bool) {
    for word in input.split_whitespace() {
        if *needs_space {
            out.push(' ');
        }
        out.push_str(word);
        *needs_space = true;
    }
}

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

pub(crate) fn inner_html_from_element_html(element_html: &str) -> String {
    let fragment = Html::parse_fragment(element_html);
    fragment
        .root_element()
        .children()
        .find_map(ElementRef::wrap)
        .map(|el| el.inner_html())
        .unwrap_or_default()
}

pub(crate) fn text_from_element_html(element_html: &str) -> String {
    let fragment = Html::parse_fragment(element_html);
    fragment
        .root_element()
        .children()
        .find_map(ElementRef::wrap)
        .map(|el| normalize_text_nodes(el.text()))
        .unwrap_or_default()
}

pub(crate) fn attrs_from_element_html(element_html: &str) -> HashMap<String, String> {
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
