use super::super::super::html_document::HtmlDocumentNode;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(in crate::renderer::backends::html_interactive) struct TableCell {
    pub(in crate::renderer::backends::html_interactive) tag: String,
    pub(in crate::renderer::backends::html_interactive) attributes: Vec<(String, String)>,
    pub(in crate::renderer::backends::html_interactive) children: Vec<HtmlDocumentNode>,
}

pub(in crate::renderer::backends::html_interactive) fn table_rows(
    nodes: &[HtmlDocumentNode],
) -> Vec<Vec<TableCell>> {
    let mut rows = Vec::new();
    collect_table_rows(nodes, &mut rows);
    rows
}

fn collect_table_rows(nodes: &[HtmlDocumentNode], rows: &mut Vec<Vec<TableCell>>) {
    for node in nodes {
        let HtmlDocumentNode::Element { tag, children, .. } = node else {
            continue;
        };
        if tag == "tr" {
            push_table_row(children, rows);
            continue;
        }
        collect_table_rows(children, rows);
    }
}

fn push_table_row(children: &[HtmlDocumentNode], rows: &mut Vec<Vec<TableCell>>) {
    let cells = children.iter().filter_map(table_cell).collect::<Vec<_>>();
    if !cells.is_empty() {
        rows.push(cells);
    }
}

fn table_cell(node: &HtmlDocumentNode) -> Option<TableCell> {
    let HtmlDocumentNode::Element {
        tag,
        attributes,
        children,
        ..
    } = node
    else {
        return None;
    };
    (tag == "th" || tag == "td").then(|| TableCell {
        tag: tag.clone(),
        attributes: attributes.clone(),
        children: children.clone(),
    })
}

pub(in crate::renderer::backends::html_interactive) fn attribute<'a>(
    attributes: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

pub(in crate::renderer::backends::html_interactive) fn node_text(
    nodes: &[HtmlDocumentNode],
) -> String {
    let mut text = String::new();
    for node in nodes {
        append_node_text(node, &mut text);
    }
    text.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn append_node_text(node: &HtmlDocumentNode, text: &mut String) {
    match node {
        HtmlDocumentNode::Text(value) => text.push_str(value),
        HtmlDocumentNode::Element { tag, children, .. } => {
            if tag == "br" {
                text.push('\n');
                return;
            }
            for child in children {
                append_node_text(child, text);
            }
        }
    }
}

pub(in crate::renderer::backends::html_interactive) fn seed_input_values(
    nodes: &[HtmlDocumentNode],
    values: &mut HashMap<u64, String>,
) {
    for node in nodes {
        seed_node_input_value(node, values);
    }
}

fn seed_node_input_value(node: &HtmlDocumentNode, values: &mut HashMap<u64, String>) {
    let HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children,
    } = node
    else {
        return;
    };
    if is_input_tag(tag) && !values.contains_key(node_id) {
        values.insert(*node_id, input_initial_value(attributes));
    }
    seed_input_values(children, values);
}

pub(in crate::renderer::backends::html_interactive) fn is_input_tag(tag: &str) -> bool {
    tag == "input" || tag == "textarea"
}

pub(in crate::renderer::backends::html_interactive) fn input_initial_value(
    attributes: &[(String, String)],
) -> String {
    attribute(attributes, "value").unwrap_or("").to_string()
}
