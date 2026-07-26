use super::super::html_css::{HtmlAttributes, StaticCss};
use super::super::html_css_selector::CssAncestor;
use super::svg::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, serialize_embedded_svg};
use super::{HtmlDocument, HtmlDocumentNode};
use html5ever::Attribute;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

impl HtmlDocument {
    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_with_styles(
        &self,
        external_stylesheets: &HashMap<String, String>,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width(external_stylesheets, 1024.0)
    }

    pub(in crate::renderer::backends) fn interactive_nodes_with_styles_at_width(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
    ) -> Vec<HtmlDocumentNode> {
        let css = StaticCss::for_interactive_document_with_styles_at_width(
            &self.document,
            external_stylesheets,
            viewport_width,
        );
        self.interactive_children(&self.document, &css, &[])
    }

    fn interactive_children(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
    ) -> Vec<HtmlDocumentNode> {
        node.children
            .borrow()
            .iter()
            .filter_map(|child| self.interactive_node(child, css, ancestors))
            .collect()
    }

    pub(super) fn interactive_node(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
    ) -> Option<HtmlDocumentNode> {
        match &node.data {
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                (!text.is_empty()).then_some(HtmlDocumentNode::Text(text))
            }
            NodeData::Element { name, attrs, .. } => self.interactive_element(
                node,
                name.local.to_string().to_ascii_lowercase(),
                attributes(&attrs.borrow()),
                css,
                ancestors,
            ),
            NodeData::Document => None,
            _ => None,
        }
    }

    fn interactive_element(
        &self,
        node: &Handle,
        tag: String,
        source_attributes: HtmlAttributes,
        css: &StaticCss,
        ancestors: &[CssAncestor],
    ) -> Option<HtmlDocumentNode> {
        if is_non_rendered_tag(&tag) {
            return None;
        }
        let attributes = css.apply_with_ancestors(&tag, &source_attributes, ancestors);
        let mut child_ancestors = ancestors.to_vec();
        child_ancestors.push(CssAncestor::new(&tag, &attributes));
        let node_id = self.node_ids.get(&(Rc::as_ptr(node) as usize)).copied()?;
        if tag == "svg" {
            return Some(embedded_svg_node(node_id, tag, attributes, node));
        }
        Some(HtmlDocumentNode::Element {
            node_id,
            tag,
            attributes,
            children: self.interactive_children(node, css, &child_ancestors),
        })
    }
}

fn embedded_svg_node(
    node_id: u64,
    tag: String,
    mut attributes: HtmlAttributes,
    node: &Handle,
) -> HtmlDocumentNode {
    let root_style = attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .map(|(_, value)| value.as_str());
    attributes.push((
        EMBEDDED_SVG_MARKUP_ATTRIBUTE.to_string(),
        serialize_embedded_svg(node, root_style),
    ));
    HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children: Vec::new(),
    }
}

pub(in crate::renderer::backends) fn attributes(source: &[Attribute]) -> HtmlAttributes {
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

fn is_non_rendered_tag(tag: &str) -> bool {
    matches!(
        tag,
        "head" | "link" | "meta" | "script" | "style" | "template" | "title"
    )
}
