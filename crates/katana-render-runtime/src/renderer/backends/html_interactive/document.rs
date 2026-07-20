use super::super::html_document::HtmlDocumentNode;
use super::constants::{MIN_LAYOUT_WIDTH, TEXT_CHARACTER_WIDTH_FACTOR};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(super) struct TableCell {
    pub(super) tag: String,
    pub(super) children: Vec<HtmlDocumentNode>,
}

pub(super) fn table_rows(nodes: &[HtmlDocumentNode]) -> Vec<Vec<TableCell>> {
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
    let HtmlDocumentNode::Element { tag, children, .. } = node else {
        return None;
    };
    (tag == "th" || tag == "td").then(|| TableCell {
        tag: tag.clone(),
        children: children.clone(),
    })
}

pub(super) fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

pub(super) fn node_text(nodes: &[HtmlDocumentNode]) -> String {
    let mut text = String::new();
    for node in nodes {
        append_node_text(node, &mut text);
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_node_text(node: &HtmlDocumentNode, text: &mut String) {
    match node {
        HtmlDocumentNode::Text(value) => text.push_str(value),
        HtmlDocumentNode::Element { children, .. } => {
            for child in children {
                append_node_text(child, text);
            }
        }
    }
}

pub(super) fn seed_input_values(nodes: &[HtmlDocumentNode], values: &mut HashMap<u64, String>) {
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

pub(super) fn is_input_tag(tag: &str) -> bool {
    tag == "input" || tag == "textarea"
}

pub(super) fn input_initial_value(attributes: &[(String, String)]) -> String {
    match attribute(attributes, "value") {
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

pub(super) fn wrap_text(text: &str, width: f32, font_size: f32) -> Vec<String> {
    let capacity = text_capacity(width, font_size);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        append_word(word, capacity, &mut line, &mut lines);
    }
    finish_line(line, &mut lines);
    lines
}

fn text_capacity(width: f32, font_size: f32) -> usize {
    (width / (font_size * TEXT_CHARACTER_WIDTH_FACTOR))
        .floor()
        .max(MIN_LAYOUT_WIDTH) as usize
}

fn append_word(word: &str, capacity: usize, line: &mut String, lines: &mut Vec<String>) {
    if line.is_empty() {
        line.push_str(word);
    } else if line.chars().count() + word.chars().count() < capacity {
        line.push(' ');
        line.push_str(word);
    } else {
        lines.push(std::mem::take(line));
        line.push_str(word);
    }
}

fn finish_line(line: String, lines: &mut Vec<String>) {
    if line.is_empty() {
        lines.push(String::new());
    } else {
        lines.push(line);
    }
}

pub(super) fn css_px(value: &str) -> Option<f32> {
    css_number(value).filter(|value| *value >= 0.0)
}

fn css_number(value: &str) -> Option<f32> {
    value
        .trim()
        .strip_suffix("px")
        .unwrap_or(value.trim())
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

pub(super) fn border_color(value: &str) -> Option<String> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    parts
        .iter()
        .find(|part| part.starts_with('#') || part.starts_with("rgb"))
        .or_else(|| parts.iter().find(|part| is_named_border_color(part)))
        .map(|part| (*part).to_string())
}

fn is_named_border_color(value: &&str) -> bool {
    value.chars().all(char::is_alphabetic)
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "solid" | "dashed" | "dotted" | "double" | "none"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_helpers_preserve_rows_text_and_empty_input_defaults() {
        assert_table_rows_preserve_cells();
        assert_input_defaults_are_seeded();
    }

    fn assert_table_rows_preserve_cells() {
        let table = element(
            "table",
            vec![
                HtmlDocumentNode::Text("ignored table text".to_string()),
                element(
                    "tbody",
                    vec![element(
                        "tr",
                        vec![
                            HtmlDocumentNode::Text("ignored row text".to_string()),
                            element("th", vec![HtmlDocumentNode::Text("Feature".to_string())]),
                            element("td", vec![HtmlDocumentNode::Text("Ready".to_string())]),
                        ],
                    )],
                ),
            ],
        );
        let rows = table_rows(&[HtmlDocumentNode::Text("ignored".to_string()), table]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[0][0].tag, "th");
        assert_eq!(
            node_text(&[element(
                "span",
                vec![HtmlDocumentNode::Text("Ready".to_string())],
            )]),
            "Ready"
        );
    }

    fn assert_input_defaults_are_seeded() {
        let inputs = vec![element(
            "section",
            vec![
                element_with_id(1, "input", Vec::new()),
                element_with_id(2, "textarea", Vec::new()),
                element("p", vec![HtmlDocumentNode::Text("plain".to_string())]),
            ],
        )];
        let mut values = HashMap::new();
        seed_input_values(&inputs, &mut values);
        assert_eq!(values.len(), 2);
        assert!(values.values().all(String::is_empty));
        assert!(is_input_tag("textarea"));
        assert_eq!(input_initial_value(&[]), "");
    }

    fn element(tag: &str, children: Vec<HtmlDocumentNode>) -> HtmlDocumentNode {
        element_with_id(0, tag, children)
    }

    fn element_with_id(
        node_id: u64,
        tag: &str,
        children: Vec<HtmlDocumentNode>,
    ) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id,
            tag: tag.to_string(),
            attributes: Vec::new(),
            children,
        }
    }
}
