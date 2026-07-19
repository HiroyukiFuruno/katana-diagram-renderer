use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::constants::{DEFAULT_MARGIN, MIN_LAYOUT_WIDTH};
use super::style::CssStyle;
use super::svg::svg_header;
use super::types::{ElementRenderContext, HitTarget, LayoutContext, LayoutResult};
use std::collections::HashMap;

pub(super) struct HtmlLayoutRenderer {
    pub(super) scroll_y: f32,
    pub(super) svg: String,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) input_values: HashMap<u64, String>,
    pub(super) focused_input: Option<u64>,
}

impl HtmlLayoutRenderer {
    pub(super) fn render(
        nodes: &[HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> LayoutResult {
        let mut renderer = Self::new(viewport, scroll_y, input_values, focused_input);
        let width = (viewport.width as f32 - DEFAULT_MARGIN * 2.0).max(MIN_LAYOUT_WIDTH);
        let bottom = renderer.render_nodes(
            nodes,
            DEFAULT_MARGIN,
            DEFAULT_MARGIN,
            width,
            &CssStyle::default(),
            None,
        );
        renderer.svg.push_str("</svg>");
        LayoutResult {
            svg: renderer.svg,
            hit_targets: renderer.hit_targets,
            content_height: bottom + DEFAULT_MARGIN,
        }
    }

    fn new(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Self {
        Self {
            scroll_y,
            svg: svg_header(viewport),
            hit_targets: Vec::new(),
            input_values: input_values.clone(),
            focused_input,
        }
    }

    pub(super) fn render_nodes(
        &mut self,
        nodes: &[HtmlDocumentNode],
        x: f32,
        mut y: f32,
        width: f32,
        inherited: &CssStyle,
        details_node_id: Option<u64>,
    ) -> f32 {
        for node in nodes {
            y = self.render_node(node, x, y, width, inherited, details_node_id);
        }
        y
    }

    fn render_node(
        &mut self,
        node: &HtmlDocumentNode,
        x: f32,
        y: f32,
        width: f32,
        inherited: &CssStyle,
        details_node_id: Option<u64>,
    ) -> f32 {
        match node {
            HtmlDocumentNode::Text(text) => self.render_text(text, x, y, width, inherited),
            HtmlDocumentNode::Element {
                node_id,
                tag,
                attributes,
                children,
            } => self.render_element_node(
                *node_id,
                tag,
                attributes,
                children,
                LayoutContext::new(x, y, width, inherited, details_node_id),
            ),
        }
    }

    fn render_element_node(
        &mut self,
        node_id: u64,
        tag: &str,
        attributes: &[(String, String)],
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_element(
            ElementRenderContext {
                node_id,
                tag,
                attributes,
                children,
            },
            layout,
        )
    }

    fn render_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let style = CssStyle::from_attributes(element.attributes, layout.style);
        if style.display_none {
            return layout.y;
        }
        self.render_tag(
            element,
            LayoutContext {
                style: &style,
                ..layout
            },
        )
    }
}
