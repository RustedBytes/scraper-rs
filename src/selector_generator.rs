use std::collections::HashSet;

use tl::{NodeHandle, VDom};

use crate::tl_dom::{
    TlParser, attrs_to_map, bytes_to_string, select_handles_from_dom, xml_safe_name,
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

fn xpath_attribute_candidates(handle: NodeHandle, parser: &TlParser<'_>) -> Vec<String> {
    let Some(tag) = handle.get(parser).and_then(|node| node.as_tag()) else {
        return Vec::new();
    };
    let attrs = attrs_to_map(tag);
    let mut ordered = Vec::new();

    const PRIORITY: [&str; 9] = [
        "id",
        "data-testid",
        "data-test",
        "data-qa",
        "name",
        "class",
        "title",
        "aria-label",
        "itemprop",
    ];
    const BLACKLIST: [&str; 10] = [
        "href",
        "src",
        "onclick",
        "onload",
        "tabindex",
        "width",
        "height",
        "style",
        "size",
        "maxlength",
    ];

    for name in PRIORITY {
        if let Some(value) = attrs.get(name).filter(|value| !value.is_empty()) {
            ordered.push((name.to_string(), value.clone()));
        }
    }

    let mut remaining = attrs.into_iter().collect::<Vec<_>>();
    remaining.sort_by(|left, right| left.0.cmp(&right.0));
    for (name, value) in remaining {
        if value.is_empty()
            || PRIORITY.contains(&name.as_str())
            || BLACKLIST.contains(&name.as_str())
            || !valid_attr_name(&name)
            || xml_safe_name(&name) != name
        {
            continue;
        }
        ordered.push((name, value));
    }

    ordered.truncate(XPATH_ATTRIBUTE_BUDGET);
    let mut predicates = Vec::new();
    for (name, value) in &ordered {
        predicates.push(format!("@{name}={}", xpath_literal(value)));
    }

    // Robula+ also tries attribute sets. Bound this to pairs to avoid exponential
    // growth while keeping the useful robustness benefit for scraping workloads.
    let max_pairs = ordered.len().min(6);
    for left in 0..max_pairs {
        for right in (left + 1)..max_pairs {
            let (left_name, left_value) = &ordered[left];
            let (right_name, right_value) = &ordered[right];
            predicates.push(format!(
                "@{left_name}={} and @{right_name}={}",
                xpath_literal(left_value),
                xpath_literal(right_value)
            ));
        }
    }

    predicates
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
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    // Robula+-style breadth: start with the target, then progressively add
    // ancestor levels. At each level prefer tag, ID/attributes, attribute sets,
    // and only later positional predicates.
    let target_tag = target.get(parser)?.as_tag()?;
    let target_name = xml_safe_name(&bytes_to_string(target_tag.name()));
    let target_attrs = xpath_attribute_candidates(target, parser);

    for predicate in &target_attrs {
        push_xpath_unique(&mut out, &mut seen, format!("//{target_name}[{predicate}]"));
    }

    // Prefer semantic attributes even when the tag alone happens to be unique.
    push_xpath_unique(&mut out, &mut seen, format!("//{target_name}"));

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
    push_xpath_unique(
        &mut out,
        &mut seen,
        format!("//{target_name}[{target_position}]"),
    );

    for ancestor_index in (0..path.len().saturating_sub(1)).rev() {
        if out.len() >= XPATH_CANDIDATE_BUDGET {
            return Some(out);
        }
        let ancestor = path[ancestor_index];
        let ancestor_tag = ancestor.get(parser)?.as_tag()?;
        let ancestor_name = xml_safe_name(&bytes_to_string(ancestor_tag.name()));
        let ancestor_attrs = xpath_attribute_candidates(ancestor, parser);

        for ancestor_predicate in ancestor_attrs.iter().take(12) {
            if out.len() >= XPATH_CANDIDATE_BUDGET {
                return Some(out);
            }
            let prefix = format!("//{ancestor_name}[{ancestor_predicate}]");
            push_xpath_unique(&mut out, &mut seen, format!("{prefix}//{target_name}"));
            for target_predicate in target_attrs.iter().take(12) {
                if out.len() >= XPATH_CANDIDATE_BUDGET {
                    return Some(out);
                }
                push_xpath_unique(
                    &mut out,
                    &mut seen,
                    format!("{prefix}//{target_name}[{target_predicate}]"),
                );
            }
            push_xpath_unique(
                &mut out,
                &mut seen,
                format!("{prefix}//{target_name}[{target_position}]"),
            );
        }

        // Level expansion without attributes mirrors Robula+'s AddLevel
        // transformation and gives a stable structural option.
        push_xpath_unique(
            &mut out,
            &mut seen,
            format!("//{ancestor_name}//{target_name}"),
        );
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
