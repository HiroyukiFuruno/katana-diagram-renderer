use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{DetailsContext, ElementRenderContext, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_node(
        &mut self,
        node: &HtmlDocumentNode,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
    ) -> f32 {
        match node {
            HtmlDocumentNode::Text(text) => {
                self.render_text(text, layout.x, layout.y, layout.width, layout.style)
            }
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
                layout,
                assigned_height,
            ),
        }
    }

    fn render_flow_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
        assigned_height: Option<f32>,
    ) -> f32 {
        let mut style = CssStyle::from_element(element.tag, element.attributes, layout.style);
        style.assign_outer_width(layout.width);
        if let Some(height) = assigned_height {
            style.assign_margin_box_height(height);
        }
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
        let bottom = renderer.render_flow_node(
            node,
            LayoutContext::new(0.0, 0.0, width, inherited, details),
            None,
        );
        renderer.ensure_layout_succeeded()?;
        Ok(bottom.max(1.0))
    }
}
