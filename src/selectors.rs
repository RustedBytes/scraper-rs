use std::cell::RefCell;
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::cache::FixedCache;
use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::tl_dom::{
    parse_owned_html_unlimited, select_elements_from_dom, select_first_element_from_dom,
};

const SELECTOR_CACHE_CAPACITY: usize = 256;

thread_local! {
    static SELECTOR_CACHE: RefCell<FixedCache<Arc<str>>> =
        RefCell::new(FixedCache::new(SELECTOR_CACHE_CAPACITY));
}

#[inline]
pub(crate) fn parse_selector(css: &str) -> PyResult<Arc<str>> {
    SELECTOR_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(selector) = cache.get(css) {
            return Ok(selector.clone());
        }

        tl::parse_query_selector(css)
            .ok_or_else(|| PyValueError::new_err(format!("Invalid CSS selector {css:?}")))?;
        let css = Arc::<str>::from(css);
        cache.insert(css.to_string(), css.clone());
        Ok(css)
    })
}

#[inline]
pub(crate) fn select_fragment(html: &str, css: &str) -> PyResult<Vec<Element>> {
    parse_selector(css)?;
    let fragment = parse_owned_html_unlimited(html.to_string())?;
    select_elements_from_dom(fragment.get_ref(), css)
}

#[inline]
pub(crate) fn select_fragment_first(html: &str, css: &str) -> PyResult<Option<Element>> {
    parse_selector(css)?;
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
    parse_selector(css)?;
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
    parse_selector(css)?;
    let parsed = parse_owned_html_unlimited(html_to_parse.into_owned())?;

    select_first_element_from_dom(parsed.get_ref(), css)
}
