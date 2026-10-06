use std::collections::HashSet;

use tl::{NodeHandle, VDom};

use crate::tl_dom::{TlParser, attrs_to_map, bytes_to_string, select_handles_from_dom};

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

fn simple_candidates(
    handle: NodeHandle,
    parser: &TlParser<'_>,
) -> Vec<String> {
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
            push_unique(
                &mut out,
                &mut seen,
                format!("[{name}=\"{escaped}\"]"),
            );
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

    for (name, value) in &attrs {
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
        if bytes_to_string(tag.name()) != tag_name {
            continue;
        }
        position += 1;
        if sibling == target {
            return position;
        }
    }

    1
}

pub(crate) fn generate_xpath_selector(dom: &Dom<'_>, target: NodeHandle) -> Option<String> {
    let parser = dom.parser();
    let path = find_path(dom, target)?;
    let mut xpath = String::new();

    for (index, handle) in path.iter().copied().enumerate() {
        let tag = handle.get(parser)?.as_tag()?;
        let tag_name = bytes_to_string(tag.name());

        let position = if index == 0 {
            same_tag_position(dom.children().iter().copied(), handle, &tag_name, parser)
        } else {
            let parent = path[index - 1].get(parser)?.as_tag()?;
            same_tag_position(
                parent.children().top().iter().copied(),
                handle,
                &tag_name,
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
