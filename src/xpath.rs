use std::cell::{Cell, RefCell};
use std::rc::Rc;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use xee_xpath::context::StaticContextBuilder;
use xee_xpath::query::SequenceQuery;
use xee_xpath::{DocumentHandle, Documents, Itemable, Queries, Query};

use crate::cache::FixedCache;
use crate::element::Element;
use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};
use crate::tl_dom::normalized_document_html;

const XPATH_CACHE_CAPACITY: usize = 128;

thread_local! {
    static XPATH_CACHE: RefCell<FixedCache<Rc<SequenceQuery>>> =
        RefCell::new(FixedCache::new(XPATH_CACHE_CAPACITY));
}

pub(crate) fn parse_xpath_documents(
    html: &str,
    parse_target: &str,
) -> PyResult<(Documents, DocumentHandle)> {
    let normalized = normalized_document_html(html);
    let mut documents = Documents::new();
    let document_handle = documents.add_string_without_uri(&normalized).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse {parse_target} for XPath evaluation: {e}"
        ))
    })?;
    Ok((documents, document_handle))
}

pub(crate) fn compile_xpath(expr: &str) -> PyResult<Rc<SequenceQuery>> {
    XPATH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(query) = cache.get(expr) {
            return Ok(query.clone());
        }

        let queries = Queries::new(StaticContextBuilder::default());
        let query = queries
            .sequence(expr)
            .map_err(|e| PyValueError::new_err(format!("Invalid XPath {expr:?}: {e:?}")))?;
        let query = Rc::new(query);
        cache.insert(expr.to_string(), query.clone());
        Ok(query)
    })
}

fn execute_xpath_sequence(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<xee_xpath::Sequence> {
    let query = compile_xpath(expr)?;
    query
        .execute(documents, context_item)
        .map_err(|e| PyValueError::new_err(format!("Failed to evaluate XPath {expr:?}: {e:?}")))
}

fn evaluate_xpath_sequence_elements(
    sequence: &xee_xpath::Sequence,
    documents: &Documents,
    expr: &str,
) -> PyResult<Vec<Element>> {
    let xot = documents.xot();
    let element_nodes = sequence.elements(xot).map_err(|e| {
        PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
    })?;

    element_nodes
        .map(|node| {
            let node = node.map_err(|e| {
                PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
            })?;
            let element = xot.element(node).ok_or_else(|| {
                PyValueError::new_err("XPath expression must return element nodes for conversion")
            })?;
            let tag = xot.local_name_str(element.name()).to_string();
            let outer_html = xot.to_string(node).map_err(|e| {
                PyValueError::new_err(format!("Failed to serialize XPath element result: {e}"))
            })?;

            Ok(Element::from_parts(tag, outer_html))
        })
        .collect()
}

fn evaluate_xpath_sequence_first_element(
    sequence: &xee_xpath::Sequence,
    documents: &Documents,
    expr: &str,
) -> PyResult<Option<Element>> {
    let xot = documents.xot();
    let mut element_nodes = sequence.elements(xot).map_err(|e| {
        PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
    })?;

    let Some(node) = element_nodes.next() else {
        return Ok(None);
    };
    let node = node.map_err(|e| {
        PyValueError::new_err(format!("XPath {expr:?} must return element nodes: {e:?}"))
    })?;
    let element = xot.element(node).ok_or_else(|| {
        PyValueError::new_err("XPath expression must return element nodes for conversion")
    })?;
    let tag = xot.local_name_str(element.name()).to_string();
    let outer_html = xot.to_string(node).map_err(|e| {
        PyValueError::new_err(format!("Failed to serialize XPath element result: {e}"))
    })?;

    Ok(Some(Element::from_parts(tag, outer_html)))
}

/// A conservative proof that every candidate in the current Robula-style
/// grammar also selects another node. Work on the normalized XPath tree so
/// name collisions, entity decoding and synthetic roots have identical semantics.
fn robust_xpath_has_ambiguous_witness(
    documents: &Documents,
    document_handle: DocumentHandle,
    target_sequence: &xee_xpath::Sequence,
    budget: usize,
) -> bool {
    let xot = documents.xot();
    let Some(target) = target_sequence
        .elements(xot)
        .ok()
        .and_then(|mut nodes| nodes.next())
        .and_then(Result::ok)
    else {
        return false;
    };
    let Some(root) = documents.document_node(document_handle) else {
        return false;
    };
    let remaining = Cell::new(budget);
    let same_shape = |left, right| {
        if remaining.get() == 0 {
            return false;
        }
        remaining.set(remaining.get() - 1);
        let (Some(left_tag), Some(right_tag)) = (xot.element(left), xot.element(right)) else {
            return false;
        };
        let left_attrs = xot.attributes(left);
        let right_attrs = xot.attributes(right);
        left_tag.name() == right_tag.name()
            && left_attrs.len() == right_attrs.len()
            && left_attrs
                .iter()
                .all(|(name, value)| right_attrs.get(name) == Some(value))
    };
    let position = |node| {
        let name = xot.element(node).unwrap().name();
        xot.parent(node).and_then(|parent| {
            xot.children(parent)
                .filter(|child| xot.element(*child).is_some_and(|tag| tag.name() == name))
                .position(|child| child == node)
        })
    };
    let ancestors = xot
        .ancestors(target)
        .skip(1)
        .filter(|node| xot.element(*node).is_some())
        .collect::<Vec<_>>();
    // This is only a preflight: cap its work and use the existing search if no
    // witness is found. Truncation can only miss a proof, never accept one.
    if ancestors.len() > budget {
        return false;
    }
    let target_position = position(target);
    let target_name = xot.element(target).unwrap().name();
    for other in xot
        .descendants(root)
        .filter(|node| {
            *node != target
                && xot
                    .element(*node)
                    .is_some_and(|tag| tag.name() == target_name)
        })
        .take_while(|_| remaining.get() > 0)
    {
        if !same_shape(target, other) || target_position != position(other) {
            continue;
        }
        let mut other_ancestors = xot
            .ancestors(other)
            .skip(1)
            .filter(|node| xot.element(*node).is_some())
            .take_while(|_| remaining.get() > 0);
        // Require the target's ancestor shapes as an ordered subsequence.
        // This stronger condition is cheap and proves ambiguity for ancestor
        // attributes, attribute pairs, AddLevel and target position predicates.
        if ancestors.iter().all(|ancestor| {
            other_ancestors.any(|other_ancestor| same_shape(*ancestor, other_ancestor))
        }) {
            return true;
        }
    }
    false
}

/// Validate node identity without serializing candidate result subtrees.
pub(crate) fn find_unique_xpath_selector(
    documents: &mut Documents,
    document_handle: DocumentHandle,
    target_path: &str,
    candidates: Vec<String>,
) -> PyResult<Option<String>> {
    let target_sequence = execute_xpath_sequence(documents, document_handle, target_path)?;
    let target_nodes = target_sequence
        .elements(documents.xot())
        .map_err(|e| PyValueError::new_err(format!("Invalid selector target: {e:?}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| PyValueError::new_err(format!("Invalid selector target: {e:?}")))?;
    if target_nodes.len() != 1 {
        return Err(PyValueError::new_err(
            "Selector target path must identify exactly one node",
        ));
    }
    // Only preflight searches large enough to overflow the compilation cache.
    // The witness proves all current candidates ambiguous; otherwise search
    // exactly as before, with the same budgets, ranking and validated fallback.
    if candidates.len() > XPATH_CACHE_CAPACITY
        && robust_xpath_has_ambiguous_witness(
            documents,
            document_handle,
            &target_sequence,
            candidates.len(),
        )
    {
        return Ok(Some(target_path.to_string()));
    }
    for candidate in candidates {
        let sequence = execute_xpath_sequence(documents, document_handle, &candidate)?;
        let mut nodes = sequence
            .elements(documents.xot())
            .map_err(|e| PyValueError::new_err(format!("Invalid selector candidate: {e:?}")))?;
        let Some(first) = nodes.next() else {
            continue;
        };
        let first = first
            .map_err(|e| PyValueError::new_err(format!("Invalid selector candidate: {e:?}")))?;
        if first == target_nodes[0] && nodes.next().is_none() {
            return Ok(Some(candidate));
        }
    }
    Ok(Some(target_path.to_string()))
}

pub(crate) fn evaluate_xpath_elements(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<Vec<Element>> {
    let sequence = execute_xpath_sequence(documents, context_item, expr)?;
    evaluate_xpath_sequence_elements(&sequence, documents, expr)
}

pub(crate) fn evaluate_xpath_first_element(
    documents: &mut Documents,
    context_item: impl Itemable,
    expr: &str,
) -> PyResult<Option<Element>> {
    let sequence = execute_xpath_sequence(documents, context_item, expr)?;
    evaluate_xpath_sequence_first_element(&sequence, documents, expr)
}

pub(crate) fn xpath_with_limit(
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Vec<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    if html_to_parse.len() < html.len() {
        return evaluate_fragment_xpath_with_fallback(html_to_parse.as_ref(), expr);
    }
    match parse_xpath_documents(html_to_parse.as_ref(), "HTML document") {
        Ok((mut documents, document_handle)) => {
            evaluate_xpath_elements(&mut documents, document_handle, expr)
        }
        Err(_) if truncate_on_limit => {
            evaluate_fragment_xpath_with_fallback(html_to_parse.as_ref(), expr)
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn xpath_first_with_limit(
    html: &str,
    expr: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Option<Element>> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    if html_to_parse.len() < html.len() {
        return evaluate_fragment_xpath_first_with_fallback(html_to_parse.as_ref(), expr);
    }
    match parse_xpath_documents(html_to_parse.as_ref(), "HTML document") {
        Ok((mut documents, document_handle)) => {
            evaluate_xpath_first_element(&mut documents, document_handle, expr)
        }
        Err(_) if truncate_on_limit => {
            evaluate_fragment_xpath_first_with_fallback(html_to_parse.as_ref(), expr)
        }
        Err(err) => Err(err),
    }
}

pub(crate) fn evaluate_fragment_xpath(html: &str, expr: &str) -> PyResult<Vec<Element>> {
    let mut wrapped = String::with_capacity(html.len() + "<xpath-fragment></xpath-fragment>".len());
    wrapped.push_str("<xpath-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</xpath-fragment>");
    let (mut documents, document_handle) = parse_xpath_documents(&wrapped, "HTML fragment")?;
    let root = documents.document_node(document_handle).ok_or_else(|| {
        PyValueError::new_err("Failed to parse HTML fragment for XPath evaluation")
    })?;
    let root_element = documents.xot().document_element(root).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse HTML fragment for XPath evaluation: {e}"
        ))
    })?;

    evaluate_xpath_elements(&mut documents, root_element, expr)
}

pub(crate) fn evaluate_fragment_xpath_first(html: &str, expr: &str) -> PyResult<Option<Element>> {
    let mut wrapped = String::with_capacity(html.len() + "<xpath-fragment></xpath-fragment>".len());
    wrapped.push_str("<xpath-fragment>");
    wrapped.push_str(html);
    wrapped.push_str("</xpath-fragment>");
    let (mut documents, document_handle) = parse_xpath_documents(&wrapped, "HTML fragment")?;
    let root = documents.document_node(document_handle).ok_or_else(|| {
        PyValueError::new_err("Failed to parse HTML fragment for XPath evaluation")
    })?;
    let root_element = documents.xot().document_element(root).map_err(|e| {
        PyValueError::new_err(format!(
            "Failed to parse HTML fragment for XPath evaluation: {e}"
        ))
    })?;

    evaluate_xpath_first_element(&mut documents, root_element, expr)
}

pub(crate) fn evaluate_fragment_xpath_with_fallback(
    html: &str,
    expr: &str,
) -> PyResult<Vec<Element>> {
    evaluate_fragment_xpath(html, expr)
}

pub(crate) fn evaluate_fragment_xpath_first_with_fallback(
    html: &str,
    expr: &str,
) -> PyResult<Option<Element>> {
    evaluate_fragment_xpath_first(html, expr)
}

pub(crate) struct XPathDocumentState {
    pub(crate) documents: Documents,
    pub(crate) document_handle: DocumentHandle,
}

#[cfg(test)]
mod selector_preflight_tests {
    use super::*;
    use crate::selector_generator::{
        generate_absolute_xpath_selector, generate_robust_xpath_candidates,
    };
    use crate::tl_dom::{parse_owned_html_unlimited, select_handles_from_dom};

    #[test]
    fn attribute_heavy_ambiguity_skips_candidate_evaluations() {
        // The selector benchmark's mirrored branches exhaust the candidate
        // budget. Prove that no candidate is compiled/evaluated, without a
        // noisy wall-clock threshold on shared CI hardware.
        for (depth, attributes) in [(4, 128), (64, 16)] {
            XPATH_CACHE.with(|cache| {
                *cache.borrow_mut() = FixedCache::new(XPATH_CACHE_CAPACITY);
            });
            let branch = |label: &str| {
                let mut html = String::new();
                for level in 0..depth {
                    html.push_str("<div");
                    for attr in 0..attributes {
                        html.push_str(&format!(" data-a{attr}=\"level-{level}-{attr}\""));
                    }
                    html.push('>');
                }
                html.push_str("<span");
                for attr in 0..attributes {
                    html.push_str(&format!(" data-a{attr}=\"leaf-{attr}\""));
                }
                html.push_str(&format!(">{label}</span>"));
                html.push_str(&"</div>".repeat(depth));
                html
            };
            let html = format!("<main>{}{}</main>", branch("decoy"), branch("target"));
            let owned = parse_owned_html_unlimited(html.clone()).unwrap();
            let dom = owned.get_ref();
            let target = select_handles_from_dom(dom, "span").unwrap()[1];
            let path = generate_absolute_xpath_selector(dom, target).unwrap();
            let candidates = generate_robust_xpath_candidates(dom, target).unwrap();
            let (mut documents, handle) = parse_xpath_documents(&html, "test").unwrap();
            for _ in 0..2 {
                assert_eq!(
                    find_unique_xpath_selector(&mut documents, handle, &path, candidates.clone(),)
                        .unwrap(),
                    Some(path.clone())
                );
                XPATH_CACHE.with(|cache| {
                    let cache = cache.borrow();
                    assert!(cache.get(&path).is_some());
                    for candidate in &candidates {
                        assert!(cache.get(candidate).is_none(),
                            "candidate evaluated: depth={depth}, attributes={attributes}, {candidate}");
                    }
                });
            }
            let target_sequence = execute_xpath_sequence(&mut documents, handle, &path).unwrap();
            assert!(!robust_xpath_has_ambiguous_witness(
                &documents,
                handle,
                &target_sequence,
                1
            ));
            // Guard the proof against future changes to the candidate grammar.
            for candidate in candidates {
                let sequence = execute_xpath_sequence(&mut documents, handle, &candidate).unwrap();
                assert!(
                    sequence.elements(documents.xot()).unwrap().count() > 1,
                    "witness incorrectly pruned a unique candidate: {candidate}"
                );
            }
        }
    }
}
