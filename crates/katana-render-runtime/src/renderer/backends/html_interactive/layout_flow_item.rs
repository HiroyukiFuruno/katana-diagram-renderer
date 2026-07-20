use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{DetailsContext, ElementRenderContext, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_node(
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
            } => self.render_flow_element(
                ElementRenderContext {
                    node_id: *node_id,
                    tag,
                    attributes,
                    children,
                },
                LayoutContext::new(x, y, width, inherited, details),
            ),
        }
    }

    fn render_flow_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let mut style = CssStyle::from_attributes(element.attributes, layout.style);
        style.consume_assigned_flow_width();
        self.render_styled_element(
            element,
            LayoutContext {
                style: &style,
                ..layout
            },
        )
    }

    pub(super) fn measure_flow_node_height(
        &self,
        node: &HtmlDocumentNode,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let viewport = HtmlBrowserViewport {
            width: width.ceil().max(1.0) as u32,
            height: 1,
            device_scale_factor: 1.0,
        };
        let mut renderer = Self::new(viewport, 0.0, &self.input_values, self.focused_input);
        let bottom = renderer.render_flow_node(node, 0.0, 0.0, width, inherited, details);
        renderer.ensure_layout_succeeded()?;
        Ok(bottom.max(1.0))
    }
}
