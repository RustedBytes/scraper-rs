use pyo3::prelude::*;

use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::tl_dom::{
    parse_owned_html_unlimited, select_elements_from_dom, select_first_element_from_dom,
};

#[inline]
pub(crate) fn select_fragment(html: &str, css: &str) -> PyResult<Vec<Element>> {
    let fragment = parse_owned_html_unlimited(html.to_string())?;
    select_elements_from_dom(fragment.get_ref(), css)
}

#[inline]
pub(crate) fn select_fragment_first(html: &str, css: &str) -> PyResult<Option<Element>> {
    let fragment = parse_owned_html_unlimited(html.to_string())?;
    select_first_element_from_dom(fragment.get_ref(), css)
}

#[inline]
pub(crate) fn select_with_limit(
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let parsed = parse_owned_html_unlimited(html_to_parse.into_owned())?;

    select_elements_from_dom(parsed.get_ref(), css)
}

#[inline]
pub(crate) fn select_first_with_limit(
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let parsed = parse_owned_html_unlimited(html_to_parse.into_owned())?;

    select_first_element_from_dom(parsed.get_ref(), css)
}
