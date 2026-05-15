use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::TreeSink;
use html5ever::{
    Attribute, ParseOpts, QualName, local_name, ns, parse_document as html5ever_parse_document,
    parse_fragment as html5ever_parse_fragment,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::limits::{DEFAULT_MAX_PARSE_BYTES, ensure_within_size_limit};

#[derive(Clone)]
enum Html5DictNodeData {
    Document,
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        template_contents: Option<usize>,
    },
    ProcessingInstruction {
        target: String,
        data: String,
    },
}

#[derive(Clone)]
struct Html5DictNode {
    parent: Option<usize>,
    children: Vec<usize>,
    data: Html5DictNodeData,
}

#[derive(Clone, Copy)]
enum Html5ParseKind {
    Document,
    Fragment,
}

#[derive(Debug)]
struct Html5ElemName {
    ns: html5ever::Namespace,
    local_name: html5ever::LocalName,
}

impl html5ever::interface::ElemName for Html5ElemName {
    fn ns(&self) -> &html5ever::Namespace {
        &self.ns
    }

    fn local_name(&self) -> &html5ever::LocalName {
        &self.local_name
    }
}

pub(crate) struct Html5DictTree {
    nodes: Vec<Html5DictNode>,
    quirks_mode: QuirksMode,
    errors: Vec<String>,
    parse_kind: Html5ParseKind,
}

struct Html5DictSink {
    nodes: RefCell<Vec<Html5DictNode>>,
    quirks_mode: std::cell::Cell<QuirksMode>,
    errors: RefCell<Vec<String>>,
    parse_kind: Html5ParseKind,
}

impl Html5DictSink {
    fn new(parse_kind: Html5ParseKind) -> Self {
        Self {
            nodes: RefCell::new(vec![Html5DictNode {
                parent: None,
                children: Vec::new(),
                data: Html5DictNodeData::Document,
            }]),
            quirks_mode: std::cell::Cell::new(QuirksMode::NoQuirks),
            errors: RefCell::new(Vec::new()),
            parse_kind,
        }
    }

    fn create_node(&self, data: Html5DictNodeData) -> usize {
        let mut nodes = self.nodes.borrow_mut();
        let id = nodes.len();
        nodes.push(Html5DictNode {
            parent: None,
            children: Vec::new(),
            data,
        });
        id
    }

    fn remove_from_parent_impl(nodes: &mut [Html5DictNode], target: usize) {
        let Some(parent) = nodes[target].parent.take() else {
            return;
        };
        if let Some(position) = nodes[parent]
            .children
            .iter()
            .position(|&child| child == target)
        {
            nodes[parent].children.remove(position);
        }
    }

    fn append_text_node(&self, nodes: &mut Vec<Html5DictNode>, parent: usize, text: StrTendril) {
        if let Some(&last_child) = nodes[parent].children.last()
            && let Html5DictNodeData::Text { text: existing } = &mut nodes[last_child].data
        {
            existing.push_str(text.as_ref());
            return;
        }

        let id = nodes.len();
        nodes.push(Html5DictNode {
            parent: Some(parent),
            children: Vec::new(),
            data: Html5DictNodeData::Text {
                text: text.to_string(),
            },
        });
        nodes[parent].children.push(id);
    }

    fn append_node(&self, nodes: &mut [Html5DictNode], parent: usize, child: usize) {
        Self::remove_from_parent_impl(nodes, child);
        nodes[child].parent = Some(parent);
        nodes[parent].children.push(child);
    }
}

impl TreeSink for Html5DictSink {
    type Handle = usize;
    type Output = Html5DictTree;
    type ElemName<'a>
        = Html5ElemName
    where
        Self: 'a;

    fn finish(self) -> Self::Output {
        Html5DictTree {
            nodes: self.nodes.into_inner(),
            quirks_mode: self.quirks_mode.get(),
            errors: self.errors.into_inner(),
            parse_kind: self.parse_kind,
        }
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.errors.borrow_mut().push(msg.into_owned());
    }

    fn get_document(&self) -> Self::Handle {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> Self::ElemName<'a> {
        let nodes = self.nodes.borrow();
        match &nodes[*target].data {
            Html5DictNodeData::Element { name, .. } => Html5ElemName {
                ns: name.ns.clone(),
                local_name: name.local.clone(),
            },
            _ => panic!("elem_name called on a non-element node"),
        }
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Self::Handle {
        let template_contents = if flags.template {
            Some(self.create_node(Html5DictNodeData::Document))
        } else {
            None
        };

        self.create_node(Html5DictNodeData::Element {
            name,
            attrs,
            template_contents,
        })
    }

    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        self.create_node(Html5DictNodeData::Comment {
            text: text.to_string(),
        })
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Self::Handle {
        self.create_node(Html5DictNodeData::ProcessingInstruction {
            target: target.to_string(),
            data: data.to_string(),
        })
    }

    fn append(&self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        let mut nodes = self.nodes.borrow_mut();
        match child {
            NodeOrText::AppendText(text) => self.append_text_node(&mut nodes, *parent, text),
            NodeOrText::AppendNode(child) => self.append_node(&mut nodes, *parent, child),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        let has_parent = self.nodes.borrow()[*element].parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let id = self.create_node(Html5DictNodeData::Doctype {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        });
        self.append(&0, NodeOrText::AppendNode(id));
    }

    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        let nodes = self.nodes.borrow();
        match &nodes[*target].data {
            Html5DictNodeData::Element {
                template_contents: Some(contents),
                ..
            } => *contents,
            _ => panic!("get_template_contents called on a non-template element"),
        }
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks_mode.set(mode);
    }

    fn append_before_sibling(&self, sibling: &Self::Handle, new_node: NodeOrText<Self::Handle>) {
        let mut nodes = self.nodes.borrow_mut();
        let Some(parent) = nodes[*sibling].parent else {
            return;
        };
        let Some(position) = nodes[parent]
            .children
            .iter()
            .position(|&child| child == *sibling)
        else {
            return;
        };

        match new_node {
            NodeOrText::AppendText(text) => {
                if position > 0 {
                    let previous = nodes[parent].children[position - 1];
                    if let Html5DictNodeData::Text { text: existing } = &mut nodes[previous].data {
                        existing.push_str(text.as_ref());
                        return;
                    }
                }

                if let Html5DictNodeData::Text { text: existing } = &mut nodes[*sibling].data {
                    existing.insert_str(0, text.as_ref());
                    return;
                }

                let id = nodes.len();
                nodes.push(Html5DictNode {
                    parent: Some(parent),
                    children: Vec::new(),
                    data: Html5DictNodeData::Text {
                        text: text.to_string(),
                    },
                });
                nodes[parent].children.insert(position, id);
            }
            NodeOrText::AppendNode(child) => {
                Self::remove_from_parent_impl(&mut nodes, child);
                nodes[child].parent = Some(parent);
                let Some(new_position) = nodes[parent]
                    .children
                    .iter()
                    .position(|&node| node == *sibling)
                else {
                    return;
                };
                nodes[parent].children.insert(new_position, child);
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        let Html5DictNodeData::Element {
            attrs: existing_attrs,
            ..
        } = &mut nodes[*target].data
        else {
            panic!("add_attrs_if_missing called on a non-element node");
        };

        for attr in attrs {
            if existing_attrs
                .iter()
                .all(|existing| existing.name != attr.name)
            {
                existing_attrs.push(attr);
            }
        }
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        let mut nodes = self.nodes.borrow_mut();
        Self::remove_from_parent_impl(&mut nodes, *target);
    }

    fn reparent_children(&self, node: &Self::Handle, new_parent: &Self::Handle) {
        let children = {
            let mut nodes = self.nodes.borrow_mut();
            std::mem::take(&mut nodes[*node].children)
        };

        for child in children {
            self.append(new_parent, NodeOrText::AppendNode(child));
        }
    }
}

fn quirks_mode_name(mode: QuirksMode) -> &'static str {
    match mode {
        QuirksMode::Quirks => "quirks",
        QuirksMode::LimitedQuirks => "limited-quirks",
        QuirksMode::NoQuirks => "no-quirks",
    }
}

fn qual_name_to_string(name: &QualName) -> String {
    name.local.to_string()
}

fn attrs_to_map(attrs: &[Attribute]) -> HashMap<String, String> {
    attrs
        .iter()
        .map(|attr| (attr.name.local.to_string(), attr.value.to_string()))
        .collect()
}

fn html5_dict_node_to_py(
    py: Python<'_>,
    tree: &Html5DictTree,
    node_id: usize,
    root_override: Option<&str>,
) -> PyResult<Py<PyDict>> {
    let node = &tree.nodes[node_id];
    let dict = PyDict::new(py);

    let node_type = root_override.unwrap_or(match &node.data {
        Html5DictNodeData::Document => "document",
        Html5DictNodeData::Doctype { .. } => "doctype",
        Html5DictNodeData::Text { .. } => "text",
        Html5DictNodeData::Comment { .. } => "comment",
        Html5DictNodeData::Element { .. } => "element",
        Html5DictNodeData::ProcessingInstruction { .. } => "processing_instruction",
    });
    dict.set_item("node_type", node_type)?;

    match &node.data {
        Html5DictNodeData::Document => {}
        Html5DictNodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            dict.set_item("name", name)?;
            dict.set_item("public_id", public_id)?;
            dict.set_item("system_id", system_id)?;
        }
        Html5DictNodeData::Text { text } | Html5DictNodeData::Comment { text } => {
            dict.set_item("text", text)?;
        }
        Html5DictNodeData::Element { name, attrs, .. } => {
            dict.set_item("tag", qual_name_to_string(name))?;
            dict.set_item("namespace", name.ns.to_string())?;
            dict.set_item("attrs", attrs_to_map(attrs))?;
        }
        Html5DictNodeData::ProcessingInstruction { target, data } => {
            dict.set_item("target", target)?;
            dict.set_item("data", data)?;
        }
    }

    let child_ids = if node_id == 0 && matches!(tree.parse_kind, Html5ParseKind::Fragment) {
        let mut flattened = Vec::new();
        for &child in &node.children {
            if let Html5DictNodeData::Element { name, .. } = &tree.nodes[child].data
                && name.local.as_ref() == "html"
            {
                flattened.extend(tree.nodes[child].children.iter().copied());
            } else {
                flattened.push(child);
            }
        }
        flattened
    } else {
        node.children.clone()
    };

    let children = PyList::empty(py);
    for child in child_ids {
        children.append(html5_dict_node_to_py(py, tree, child, None)?)?;
    }
    dict.set_item("children", children)?;

    if node_id == 0 {
        dict.set_item("quirks_mode", quirks_mode_name(tree.quirks_mode))?;
        dict.set_item("errors", &tree.errors)?;
    }

    Ok(dict.into())
}

pub(crate) fn parse_document_to_dict_tree(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Html5DictTree> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    Ok(html5ever_parse_document(
        Html5DictSink::new(Html5ParseKind::Document),
        ParseOpts::default(),
    )
    .one(html_to_parse.as_ref()))
}

pub(crate) fn parse_fragment_to_dict_tree(
    html: &str,
    max_size_bytes: Option<usize>,
    truncate_on_limit: bool,
) -> PyResult<Html5DictTree> {
    let max_size_bytes = max_size_bytes.unwrap_or(DEFAULT_MAX_PARSE_BYTES);
    let html_to_parse = ensure_within_size_limit(html, max_size_bytes, truncate_on_limit)?;
    let context_name = QualName::new(None, ns!(html), local_name!("div"));
    Ok(html5ever_parse_fragment(
        Html5DictSink::new(Html5ParseKind::Fragment),
        ParseOpts::default(),
        context_name,
        Vec::new(),
        false,
    )
    .one(html_to_parse.as_ref()))
}

pub(crate) fn html5_tree_to_py_dict(py: Python<'_>, tree: Html5DictTree) -> PyResult<Py<PyDict>> {
    let root_override = match tree.parse_kind {
        Html5ParseKind::Document => Some("document"),
        Html5ParseKind::Fragment => Some("document_fragment"),
    };
    html5_dict_node_to_py(py, &tree, 0, root_override)
}
