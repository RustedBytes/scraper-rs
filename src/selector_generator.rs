use std::collections::HashSet;

use tl::{NodeHandle, VDom};

use crate::tl_dom::{
    TlParser, attrs_to_map, bytes_to_string, node_text, select_handles_from_dom, xml_safe_name,
};

// Bound both eager candidate allocation and XPath executions across all levels.
// Exhaustion falls back to the validated absolute positional path.
const XPATH_CANDIDATE_BUDGET: usize = 256;
const XPATH_ATTRIBUTE_BUDGET: usize = 12;

type Dom<'a> = VDom<'a, 32, 0, 0, 16, 16, 0>;

fn find_path_from(
    target: NodeHandle,
    handle: NodeHandle,
    parser: &TlParser<'_>,
    path: &mut Vec<NodeHandle>,
) -> bool {
    let Some(node) = handle.get(parser) else {
        return false;
    };

    if node.as_tag().is_none() {
        return false;
    }

    path.push(handle);
    if handle == target {
        return true;
    }

    if let Some(tag) = node.as_tag() {
        for child in tag.children().top().iter() {
            if find_path_from(target, *child, parser, path) {
                return true;
            }
        }
    }

    path.pop();
    false
}

fn find_path(dom: &Dom<'_>, target: NodeHandle) -> Option<Vec<NodeHandle>> {
    let parser = dom.parser();
    let mut path = Vec::new();

    for root in dom.children() {
        if find_path_from(target, *root, parser, &mut path) {
            return Some(path);
        }
    }

    None
}

fn css_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\A "),
            '\r' => out.push_str("\\D "),
            '\u{000C}' => out.push_str("\\C "),
            _ => out.push(ch),
        }
    }
    out
}

fn valid_attr_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn push_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, value: String) {
    if seen.insert(value.clone()) {
        out.push(value);
    }
}

fn simple_candidates(handle: NodeHandle, parser: &TlParser<'_>) -> Vec<String> {
    let Some(tag) = handle.get(parser).and_then(|node| node.as_tag()) else {
        return Vec::new();
    };

    let tag_name = bytes_to_string(tag.name());
    let attrs = attrs_to_map(tag);
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    const PREFERRED: [&str; 12] = [
        "id",
        "data-testid",
        "data-test",
        "data-qa",
        "name",
        "aria-label",
        "itemprop",
        "role",
        "href",
        "src",
        "title",
        "alt",
    ];

    for name in PREFERRED {
        if let Some(value) = attrs.get(name).filter(|value| !value.is_empty()) {
            let escaped = css_string(value);
            push_unique(&mut out, &mut seen, format!("[{name}=\"{escaped}\"]"));
            push_unique(
                &mut out,
                &mut seen,
                format!("{tag_name}[{name}=\"{escaped}\"]"),
            );
        }
    }

    if let Some(class_value) = attrs.get("class") {
        for class_name in class_value.split_whitespace().take(4) {
            let escaped = css_string(class_name);
            push_unique(
                &mut out,
                &mut seen,
                format!("{tag_name}[class~=\"{escaped}\"]"),
            );
        }
        if !class_value.is_empty() {
            let escaped = css_string(class_value);
            push_unique(
                &mut out,
                &mut seen,
                format!("{tag_name}[class=\"{escaped}\"]"),
            );
        }
    }

    let mut remaining = attrs.iter().collect::<Vec<_>>();
    remaining.sort_by(|left, right| left.0.cmp(right.0));
    for (name, value) in remaining {
        if value.is_empty() || !valid_attr_name(name) || PREFERRED.contains(&name.as_str()) {
            continue;
        }
        let escaped = css_string(value);
        push_unique(
            &mut out,
            &mut seen,
            format!("{tag_name}[{name}=\"{escaped}\"]"),
        );
    }

    push_unique(&mut out, &mut seen, tag_name);
    out
}

fn is_unique_css(dom: &Dom<'_>, target: NodeHandle, selector: &str) -> bool {
    match select_handles_from_dom(dom, selector) {
        Ok(handles) => handles.len() == 1 && handles[0] == target,
        Err(_) => false,
    }
}

pub(crate) fn generate_css_selector(dom: &Dom<'_>, target: NodeHandle) -> Option<String> {
    let parser = dom.parser();
    let path = find_path(dom, target)?;
    let target_candidates = simple_candidates(target, parser);

    for candidate in &target_candidates {
        if is_unique_css(dom, target, candidate) {
            return Some(candidate.clone());
        }
    }

    for target_candidate in target_candidates.iter().take(8) {
        let mut suffix = target_candidate.clone();

        for ancestor in path.iter().rev().skip(1) {
            let ancestor_candidates = simple_candidates(*ancestor, parser);
            let best = ancestor_candidates.last()?.clone();

            for ancestor_candidate in ancestor_candidates.iter().take(6) {
                let candidate = format!("{ancestor_candidate} > {suffix}");
                if is_unique_css(dom, target, &candidate) {
                    return Some(candidate);
                }
            }

            suffix = format!("{best} > {suffix}");
            if is_unique_css(dom, target, &suffix) {
                return Some(suffix.clone());
            }
        }
    }

    None
}

fn same_tag_position(
    siblings: impl Iterator<Item = NodeHandle>,
    target: NodeHandle,
    tag_name: &str,
    parser: &TlParser<'_>,
) -> usize {
    let mut position = 0;

    for sibling in siblings {
        let Some(tag) = sibling.get(parser).and_then(|node| node.as_tag()) else {
            continue;
        };
        if xml_safe_name(&bytes_to_string(tag.name())) != xml_safe_name(tag_name) {
            continue;
        }
        position += 1;
        if sibling == target {
            return position;
        }
    }

    1
}

fn xpath_literal(value: &str) -> String {
    if !value.contains('\'') {
        return format!("'{value}'");
    }
    if !value.contains('"') {
        return format!("\"{value}\"");
    }

    let parts = value
        .split('\'')
        .map(|part| format!("'{part}'"))
        .collect::<Vec<_>>();
    format!("concat({})", parts.join(", \"'\", "))
}

#[derive(Debug)]
struct RankedXPathPredicate {
    score: u16,
    predicate: String,
}

fn looks_generated_identifier(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("css-")
        || lower.starts_with("sc-")
        || lower.contains("css-module")
        || lower.contains("__")
    {
        return true;
    }

    let bytes = value.as_bytes();
    let mut longest_digit_run = 0;
    let mut current_digit_run = 0;
    for byte in bytes {
        if byte.is_ascii_digit() {
            current_digit_run += 1;
            longest_digit_run = longest_digit_run.max(current_digit_run);
        } else {
            current_digit_run = 0;
        }
    }

    longest_digit_run >= 5
        || (value.len() >= 12
            && value
                .bytes()
                .filter(|byte| byte.is_ascii_hexdigit())
                .count()
                >= value.len().saturating_sub(2))
}

fn stable_class_tokens(value: &str) -> Vec<&str> {
    value
        .split_whitespace()
        .filter(|token| {
            !looks_generated_identifier(token)
                && token.len() >= 2
                && token
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
        .take(4)
        .collect()
}

fn href_path(value: &str) -> Option<String> {
    let without_fragment = value.split('#').next().unwrap_or(value);
    let without_query = without_fragment.split('?').next().unwrap_or(without_fragment);

    if let Some(scheme_index) = without_query.find("://") {
        let after_scheme = &without_query[(scheme_index + 3)..];
        let slash = after_scheme.find('/')?;
        let path = &after_scheme[slash..];
        return (!path.is_empty()).then(|| path.to_string());
    }

    (!without_query.is_empty()).then(|| without_query.to_string())
}

fn push_ranked_attribute(
    out: &mut Vec<RankedXPathPredicate>,
    score: u16,
    name: &str,
    value: &str,
) {
    if value.is_empty() || !valid_attr_name(name) || xml_safe_name(name) != name {
        return;
    }
    out.push(RankedXPathPredicate {
        score,
        predicate: format!("@{name}={}", xpath_literal(value)),
    });
}

fn xpath_attribute_candidates(handle: NodeHandle, parser: &TlParser<'_>) -> Vec<RankedXPathPredicate> {
    let Some(tag) = handle.get(parser).and_then(|node| node.as_tag()) else {
        return Vec::new();
    };
    let attrs = attrs_to_map(tag);
    let tag_name = bytes_to_string(tag.name()).to_ascii_lowercase();
    let mut out = Vec::new();

    // Structural/content semantics used by scraping sites tend to survive layout
    // churn better than test hooks, generated IDs, or presentation classes.
    let mut data_attrs = attrs
        .iter()
        .filter(|(name, value)| {
            name.starts_with("data-")
                && !value.is_empty()
                && !matches!(
                    name.as_str(),
                    "data-testid" | "data-test" | "data-qa"
                )
                && !name.contains("render")
                && !name.contains("random")
                && !name.contains("build")
                && !name.contains("react")
        })
        .collect::<Vec<_>>();
    data_attrs.sort_by(|left, right| left.0.cmp(right.0));
    for (name, value) in data_attrs {
        push_ranked_attribute(&mut out, 100, name, value);
    }

    for name in ["itemprop", "name", "aria-label", "role"] {
        if let Some(value) = attrs.get(name) {
            push_ranked_attribute(&mut out, 95, name, value);
        }
    }

    if let Some(value) = attrs.get("id").filter(|value| !value.is_empty()) {
        push_ranked_attribute(
            &mut out,
            if looks_generated_identifier(value) { 25 } else { 92 },
            "id",
            value,
        );
    }

    // Test hooks remain useful, but rank below page/domain semantics so a
    // semantic ancestor can win when both uniquely identify the target.
    for name in ["data-testid", "data-test", "data-qa"] {
        if let Some(value) = attrs.get(name) {
            push_ranked_attribute(&mut out, 90, name, value);
        }
    }

    if matches!(tag_name.as_str(), "a" | "area") {
        if let Some(value) = attrs.get("href").filter(|value| !value.is_empty()) {
            if let Some(path) = href_path(value) {
                // Path-based match survives query-parameter churn and
                // relative -> absolute URL changes.
                out.push(RankedXPathPredicate {
                    score: 96,
                    predicate: format!(
                        "ends-with(substring-before(concat(@href, '?'), '?'), {})",
                        xpath_literal(&path)
                    ),
                });
            }
            push_ranked_attribute(&mut out, 80, "href", value);
        }
    }

    if matches!(
        tag_name.as_str(),
        "img" | "source" | "video" | "audio" | "iframe" | "script"
    ) {
        if let Some(value) = attrs.get("src").filter(|value| !value.is_empty()) {
            if let Some(path) = href_path(value) {
                out.push(RankedXPathPredicate {
                    score: 85,
                    predicate: format!(
                        "ends-with(substring-before(concat(@src, '?'), '?'), {})",
                        xpath_literal(&path)
                    ),
                });
            }
            push_ranked_attribute(&mut out, 70, "src", value);
        }
    }

    for name in ["title", "alt"] {
        if let Some(value) = attrs.get(name) {
            push_ranked_attribute(&mut out, 60, name, value);
        }
    }

    if let Some(value) = attrs.get("class").filter(|value| !value.is_empty()) {
        for token in stable_class_tokens(value) {
            out.push(RankedXPathPredicate {
                score: 50,
                predicate: format!(
                    "contains(concat(' ', normalize-space(@class), ' '), {})",
                    xpath_literal(&format!(" {token} "))
                ),
            });
        }
        out.push(RankedXPathPredicate {
            score: 30,
            predicate: format!("@class={}", xpath_literal(value)),
        });
    }

    let mut remaining = attrs.into_iter().collect::<Vec<_>>();
    remaining.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, value) in remaining {
        if value.is_empty()
            || name == "id"
            || name == "class"
            || name == "href"
            || name == "src"
            || name.starts_with("data-")
            || matches!(
                name.as_str(),
                "itemprop" | "name" | "aria-label" | "role" | "title" | "alt"
            )
            || matches!(
                name.as_str(),
                "onclick"
                    | "onload"
                    | "tabindex"
                    | "width"
                    | "height"
                    | "style"
                    | "size"
                    | "maxlength"
            )
            || !valid_attr_name(&name)
            || xml_safe_name(&name) != name
        {
            continue;
        }
        push_ranked_attribute(&mut out, 65, &name, &value);
    }

    out.sort_by(|left, right| right.score.cmp(&left.score));
    out.truncate(XPATH_ATTRIBUTE_BUDGET);

    // Bounded pair predicates retain the Robula+ attribute-set behavior without
    // letting attribute-heavy targets crowd semantic ancestor anchors out of
    // the global candidate budget.
    let singles_len = out.len().min(4);
    let singles = out
        .iter()
        .take(singles_len)
        .map(|candidate| (candidate.score, candidate.predicate.clone()))
        .collect::<Vec<_>>();
    for left in 0..singles.len() {
        for right in (left + 1)..singles.len() {
            out.push(RankedXPathPredicate {
                score: singles[left].0.min(singles[right].0).saturating_sub(1),
                predicate: format!("{} and {}", singles[left].1, singles[right].1),
            });
        }
    }

    out.sort_by(|left, right| right.score.cmp(&left.score));
    out
}

fn target_text_candidate(
    target: NodeHandle,
    parser: &TlParser<'_>,
) -> Option<RankedXPathPredicate> {
    let node = target.get(parser)?;
    let tag = node.as_tag()?;

    // Text is a useful last-resort identity for otherwise attribute-less
    // content nodes. Do not let it bypass the bounded attribute search for
    // attribute-heavy elements.
    if !attrs_to_map(tag).is_empty() {
        return None;
    }

    let text = node_text(node, parser);
    let normalized = text.trim();
    if normalized.is_empty() || normalized.chars().count() > 160 {
        return None;
    }

    Some(RankedXPathPredicate {
        score: 75,
        predicate: format!("normalize-space(.)={}", xpath_literal(normalized)),
    })
}


fn push_xpath_unique(out: &mut Vec<String>, seen: &mut HashSet<String>, value: String) {
    if out.len() < XPATH_CANDIDATE_BUDGET {
        push_unique(out, seen, value);
    }
}

pub(crate) fn generate_robust_xpath_candidates(
    dom: &Dom<'_>,
    target: NodeHandle,
) -> Option<Vec<String>> {
    let parser = dom.parser();
    let path = find_path(dom, target)?;
    let target_tag = target.get(parser)?.as_tag()?;
    let target_name = xml_safe_name(&bytes_to_string(target_tag.name()));
    let target_attrs = xpath_attribute_candidates(target, parser);

    let mut ranked = Vec::<(u16, usize, String)>::new();
    let mut sequence = 0usize;
    let mut add_ranked = |score: u16, value: String| {
        ranked.push((score, sequence, value));
        sequence += 1;
    };

    // Direct target predicates.
    for candidate in &target_attrs {
        add_ranked(
            candidate.score,
            format!("//{target_name}[{}]", candidate.predicate),
        );
    }

    if let Some(text_candidate) = target_text_candidate(target, parser) {
        add_ranked(
            text_candidate.score,
            format!("//{target_name}[{}]", text_candidate.predicate),
        );
    }

    // Build ancestor anchors globally, then let semantic stability outrank
    // proximity. This prevents a nearby presentation class from beating a
    // farther stable data-section/data-zone anchor.
    for (distance, ancestor) in path
        .iter()
        .rev()
        .skip(1)
        .copied()
        .enumerate()
    {
        let ancestor_tag = ancestor.get(parser)?.as_tag()?;
        let ancestor_name = xml_safe_name(&bytes_to_string(ancestor_tag.name()));
        let ancestor_attrs = xpath_attribute_candidates(ancestor, parser);
        let distance_penalty = (distance as u16).min(8);

        for ancestor_candidate in ancestor_attrs.iter().take(XPATH_ATTRIBUTE_BUDGET) {
            let base_score = ancestor_candidate.score.saturating_sub(distance_penalty);
            let prefix = format!(
                "//{ancestor_name}[{}]",
                ancestor_candidate.predicate
            );

            add_ranked(base_score.saturating_add(1), format!("{prefix}//{target_name}"));

            // Keep cross-products intentionally small: the ancestor-only form
            // is the robust candidate we most want to preserve under budget.
            for target_candidate in target_attrs.iter().take(2) {
                add_ranked(
                    base_score.min(target_candidate.score),
                    format!(
                        "{prefix}//{target_name}[{}]",
                        target_candidate.predicate
                    ),
                );
            }
        }
    }

    // Tag-only and positional candidates are deliberately late fallbacks.
    add_ranked(20, format!("//{target_name}"));

    let target_raw_name = bytes_to_string(target_tag.name());
    let target_position = if path.len() > 1 {
        let parent = path[path.len() - 2].get(parser)?.as_tag()?;
        same_tag_position(
            parent.children().top().iter().copied(),
            target,
            &target_raw_name,
            parser,
        )
    } else {
        same_tag_position(
            dom.children().iter().copied(),
            target,
            &target_raw_name,
            parser,
        )
    };
    add_ranked(5, format!("//{target_name}[{target_position}]"));

    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.cmp(&right.1))
    });

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (_, _, candidate) in ranked {
        push_xpath_unique(&mut out, &mut seen, candidate);
        if out.len() >= XPATH_CANDIDATE_BUDGET {
            break;
        }
    }

    Some(out)
}

pub(crate) fn generate_absolute_xpath_selector(
    dom: &Dom<'_>,
    target: NodeHandle,
) -> Option<String> {
    let parser = dom.parser();
    let path = find_path(dom, target)?;
    let root_element_count = dom
        .children()
        .iter()
        .filter_map(|handle| handle.get(parser))
        .filter(|node| node.as_tag().is_some())
        .count();
    let mut xpath = if root_element_count > 1 {
        String::from("/xpath-document[1]")
    } else {
        String::new()
    };

    for (index, handle) in path.iter().copied().enumerate() {
        let tag = handle.get(parser)?.as_tag()?;
        let raw_tag_name = bytes_to_string(tag.name());
        let tag_name = xml_safe_name(&raw_tag_name);

        let position = if index == 0 {
            same_tag_position(
                dom.children().iter().copied(),
                handle,
                &raw_tag_name,
                parser,
            )
        } else {
            let parent = path[index - 1].get(parser)?.as_tag()?;
            same_tag_position(
                parent.children().top().iter().copied(),
                handle,
                &raw_tag_name,
                parser,
            )
        };

        xpath.push('/');
        xpath.push_str(&tag_name);
        xpath.push('[');
        xpath.push_str(&position.to_string());
        xpath.push(']');
    }

    Some(xpath)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tl_dom::parse_owned_html_unlimited;

    #[test]
    fn xpath_search_has_a_global_budget() {
        let mut html = String::new();
        for depth in 0..64 {
            html.push_str("<div");
            for attr in 0..32 {
                html.push_str(&format!(" data-a{attr}=\"{depth}-{attr}\""));
            }
            html.push('>');
        }
        html.push_str("<span>target</span>");
        html.push_str(&"</div>".repeat(64));
        let owned = parse_owned_html_unlimited(html).unwrap();
        let dom = owned.get_ref();
        let target = select_handles_from_dom(dom, "span").unwrap()[0];
        let candidates = generate_robust_xpath_candidates(dom, target).unwrap();
        assert_eq!(candidates.len(), XPATH_CANDIDATE_BUDGET);
        assert_eq!(
            candidates.iter().collect::<HashSet<_>>().len(),
            candidates.len()
        );
    }
}
