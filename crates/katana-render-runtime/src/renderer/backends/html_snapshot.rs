use super::html_css::{HtmlAttributes, StaticCss};
use super::html_table::render_table;
use markup5ever_rcdom::{Handle, NodeData};

const HIDDEN_ELEMENTS: &[&str] = &[
    "head", "link", "meta", "script", "style", "template", "title",
];
const STRUCTURAL_WRAPPERS: &[&str] = &["body", "html", "main"];
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "source", "track", "wbr",
];

pub(super) fn render_document(document: &Handle) -> String {
    let css = StaticCss::from_document(document);
    render_children(document, &css).join("\n\n")
}

fn render_children(node: &Handle, css: &StaticCss) -> Vec<String> {
    node.children
        .borrow()
        .iter()
        .filter_map(|child| render_node(child, css))
        .filter(|content| !content.trim().is_empty())
        .collect()
}

fn render_node(node: &Handle, css: &StaticCss) -> Option<String> {
    match &node.data {
        NodeData::Text { contents } => Some(escape_text(&contents.borrow())),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string().to_ascii_lowercase();
            if HIDDEN_ELEMENTS.contains(&tag.as_str()) {
                return None;
            }
            if STRUCTURAL_WRAPPERS.contains(&tag.as_str()) {
                return Some(render_children(node, css).join("\n\n"));
            }
            if tag == "table" {
                return Some(render_table(node));
            }
            let attributes = attributes(&attrs.borrow());
            let attributes = css.apply(&tag, &attributes);
            let opening = opening_tag(&tag, &attributes);
            if VOID_ELEMENTS.contains(&tag.as_str()) {
                return Some(opening);
            }
            let children = render_children(node, css).join("");
            Some(format!("{opening}{children}</{tag}>"))
        }
        _ => None,
    }
}

fn attributes(source: &[html5ever::Attribute]) -> HtmlAttributes {
    source
        .iter()
        .map(|attribute| {
            (
                attribute.name.local.to_string().to_ascii_lowercase(),
                attribute.value.to_string(),
            )
        })
        .collect()
}

fn opening_tag(tag: &str, attributes: &HtmlAttributes) -> String {
    let suffix = attributes
        .iter()
        .map(|(name, value)| format!(r#" {name}="{}""#, escape_attribute(value)))
        .collect::<String>();
    format!("<{tag}{suffix}>")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attribute(value: &str) -> String {
    escape_text(value).replace('"', "&quot;")
}
