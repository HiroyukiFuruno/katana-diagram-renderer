use super::super::html_css::{HtmlAttributes, StaticCss};
use super::super::html_css_selector::CssAncestor;
use super::{HtmlDocument, HtmlDocumentNode};
use html5ever::Attribute;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[path = "html_document_projection_svg.rs"]
mod embedded_svg;

use embedded_svg::embedded_svg_node;

struct InteractiveElementProjection<'a> {
    node: &'a Handle,
    tag: String,
    source_attributes: HtmlAttributes,
    css: &'a StaticCss,
    ancestors: &'a [CssAncestor],
    sibling_index: usize,
    hovered_nodes: &'a HashSet<u64>,
}

impl HtmlDocument {
    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_with_styles(
        &self,
        external_stylesheets: &HashMap<String, String>,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width(external_stylesheets, 1024.0)
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_with_styles_at_width(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
    ) -> Vec<HtmlDocumentNode> {
        self.interactive_nodes_with_styles_at_width_and_hover(
            external_stylesheets,
            viewport_width,
            &HashSet::new(),
        )
    }

    pub(in crate::renderer::backends) fn interactive_nodes_with_styles_at_width_and_hover(
        &self,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
        hovered_nodes: &HashSet<u64>,
    ) -> Vec<HtmlDocumentNode> {
        let css = StaticCss::for_interactive_document_with_styles_at_width(
            &self.document,
            external_stylesheets,
            viewport_width,
        );
        self.interactive_children(&self.document, &css, &[], hovered_nodes)
    }

    fn interactive_children(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
        hovered_nodes: &HashSet<u64>,
    ) -> Vec<HtmlDocumentNode> {
        let mut sibling_index = 0;
        node.children
            .borrow()
            .iter()
            .filter_map(|child| {
                if matches!(child.data, NodeData::Element { .. }) {
                    sibling_index += 1;
                }
                self.interactive_node(child, css, ancestors, sibling_index, hovered_nodes)
            })
            .collect()
    }

    pub(super) fn interactive_node(
        &self,
        node: &Handle,
        css: &StaticCss,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered_nodes: &HashSet<u64>,
    ) -> Option<HtmlDocumentNode> {
        match &node.data {
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                (!text.is_empty()).then_some(HtmlDocumentNode::Text(text))
            }
            NodeData::Element { name, attrs, .. } => InteractiveElementProjection {
                node,
                tag: name.local.to_string().to_ascii_lowercase(),
                source_attributes: attributes(&attrs.borrow()),
                css,
                ancestors,
                sibling_index,
                hovered_nodes,
            }
            .project(self),
            NodeData::Document => None,
            _ => None,
        }
    }
}

impl InteractiveElementProjection<'_> {
    fn project(self, document: &HtmlDocument) -> Option<HtmlDocumentNode> {
        if is_non_rendered_tag(&self.tag) {
            return None;
        }
        let node_id = document
            .node_ids
            .get(&(Rc::as_ptr(self.node) as usize))
            .copied()?;
        let hovered = self.hovered_nodes.contains(&node_id);
        let (attributes, child_ancestors) = self.attributes_and_ancestors(hovered);
        if self.tag == "svg" {
            return Some(embedded_svg_node(node_id, self.tag, attributes, self.node));
        }
        Some(HtmlDocumentNode::Element {
            node_id,
            tag: self.tag,
            attributes,
            children: document.interactive_children(
                self.node,
                self.css,
                &child_ancestors,
                self.hovered_nodes,
            ),
        })
    }

    fn attributes_and_ancestors(&self, hovered: bool) -> (HtmlAttributes, Vec<CssAncestor>) {
        let attributes = self.css.apply_with_ancestors_at_state(
            &self.tag,
            &self.source_attributes,
            self.ancestors,
            self.sibling_index,
            hovered,
        );
        let mut child_ancestors = self.ancestors.to_vec();
        child_ancestors.push(CssAncestor::new_at_state(
            &self.tag,
            &attributes,
            self.sibling_index,
            hovered,
        ));
        (attributes, child_ancestors)
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
