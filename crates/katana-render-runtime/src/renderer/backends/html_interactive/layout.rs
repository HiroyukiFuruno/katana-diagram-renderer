use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::constants::{DEFAULT_MARGIN, MIN_LAYOUT_WIDTH};
use super::document::attribute;
use super::style::CssStyle;
use super::svg::svg_header;
use super::types::{DetailsContext, ElementRenderContext, HitTarget, LayoutContext, LayoutResult};
use std::collections::HashMap;

pub(super) struct HtmlLayoutRenderer {
    pub(super) scroll_y: f32,
    pub(super) svg: String,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) anchor_positions: HashMap<String, f32>,
    pub(super) input_values: HashMap<u64, String>,
    pub(super) focused_input: Option<u64>,
    pub(super) layout_error: Option<String>,
}

impl HtmlLayoutRenderer {
    pub(super) fn render(
        nodes: &[HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Result<LayoutResult, String> {
        let mut renderer = Self::new(viewport, scroll_y, input_values, focused_input);
        let width = (viewport.logical_width() - DEFAULT_MARGIN * 2.0).max(MIN_LAYOUT_WIDTH);
        let bottom = renderer.render_nodes(
            nodes,
            DEFAULT_MARGIN,
            DEFAULT_MARGIN,
            width,
            &CssStyle::browser_default(),
            DetailsContext::NONE,
        );
        renderer.ensure_layout_succeeded()?;
        renderer.svg.push_str("</svg>");
        Ok(LayoutResult {
            svg: renderer.svg,
            hit_targets: renderer.hit_targets,
            anchor_positions: renderer.anchor_positions,
            content_height: bottom + DEFAULT_MARGIN,
        })
    }

    pub(super) fn new(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Self {
        Self {
            scroll_y,
            svg: svg_header(viewport),
            hit_targets: Vec::new(),
            anchor_positions: HashMap::new(),
            input_values: input_values.clone(),
            focused_input,
            layout_error: None,
        }
    }

    pub(super) fn render_nodes(
        &mut self,
        nodes: &[HtmlDocumentNode],
        x: f32,
        mut y: f32,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        for node in nodes {
            y = self.render_node(node, x, y, width, inherited, details);
        }
        y
    }

    pub(super) fn render_node(
        &mut self,
        node: &HtmlDocumentNode,
        x: f32,
        y: f32,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
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
                LayoutContext::new(x, y, width, inherited, details),
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
        self.render_styled_element(
            element,
            LayoutContext {
                style: &style,
                ..layout
            },
        )
    }

    pub(super) fn render_styled_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        if layout.style.display == taffy::style::Display::None {
            return layout.y;
        }
        self.record_anchor(element, layout.y);
        self.render_tag(element, layout)
    }

    fn record_anchor(&mut self, element: ElementRenderContext<'_>, y: f32) {
        let anchor = attribute(element.attributes, "id").or_else(|| {
            (element.tag == "a")
                .then(|| attribute(element.attributes, "name"))
                .flatten()
        });
        if let Some(anchor) = anchor.filter(|anchor| !anchor.is_empty()) {
            self.anchor_positions.entry(anchor.to_string()).or_insert(y);
        }
    }

    pub(super) fn ensure_layout_succeeded(&mut self) -> Result<(), String> {
        self.layout_error.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlBrowserViewport, HtmlLayoutRenderer};
    use std::collections::HashMap;

    #[test]
    fn renderer_propagates_recorded_layout_errors() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        renderer.layout_error = Some("layout failed".to_string());

        assert_eq!(
            renderer.ensure_layout_succeeded(),
            Err("layout failed".to_string())
        );
    }
}
