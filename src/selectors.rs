use std::cell::RefCell;
use std::sync::Arc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scraper::{Html, Selector};

use crate::cache::FixedCache;
use crate::element::{Element, snapshot_element};
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};

const SELECTOR_CACHE_CAPACITY: usize = 256;

thread_local! {
    static SELECTOR_CACHE: RefCell<FixedCache<Arc<Selector>>> =
        RefCell::new(FixedCache::new(SELECTOR_CACHE_CAPACITY));
}

pub(crate) fn parse_selector(css: &str) -> PyResult<Arc<Selector>> {
    SELECTOR_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(selector) = cache.get(css) {
            return Ok(selector.clone());
        }

        let selector =
            Arc::new(Selector::parse(css).map_err(|e| {
                PyValueError::new_err(format!("Invalid CSS selector {css:?}: {e:?}"))
            })?);
        cache.insert(css.to_string(), selector.clone());
        Ok(selector)
    })
}

pub(crate) fn select_fragment(html: &str, css: &str) -> PyResult<Vec<Element>> {
    let selector = parse_selector(css)?;
    let fragment = Html::parse_fragment(html);
    Ok(fragment
        .select(selector.as_ref())
        .map(snapshot_element)
        .collect())
}

pub(crate) fn select_fragment_first(html: &str, css: &str) -> PyResult<Option<Element>> {
    let selector = parse_selector(css)?;
    let fragment = Html::parse_fragment(html);
    Ok(fragment
        .select(selector.as_ref())
        .next()
        .map(snapshot_element))
}

pub(crate) fn select_with_limit(
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let selector = parse_selector(css)?;
    let parsed = Html::parse_document(html_to_parse.as_ref());

    Ok(parsed
        .select(selector.as_ref())
        .map(snapshot_element)
        .collect::<Vec<_>>())
}

pub(crate) fn select_first_with_limit(
    html: &str,
    css: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let selector = parse_selector(css)?;
    let parsed = Html::parse_document(html_to_parse.as_ref());

    Ok(parsed
        .select(selector.as_ref())
        .next()
        .map(snapshot_element))
}
