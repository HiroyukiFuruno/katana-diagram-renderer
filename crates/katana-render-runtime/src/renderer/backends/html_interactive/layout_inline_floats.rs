use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::style::{CssFloat, CssStyle};
use super::measure::InlineMeasurement;
use super::state::InlineFlowState;

pub(in crate::renderer::backends::html_interactive) struct InlineFloat;

impl InlineFloat {
    pub(super) fn node_geometry(
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        available: f32,
    ) -> Option<(CssFloat, f32)> {
        let HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } = node
        else {
            return None;
        };
        let style = CssStyle::from_element(tag, attributes, inherited);
        if style.float == CssFloat::None || style.display == taffy::style::Display::None {
            return None;
        }
        let available_box =
            (available - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let box_width = style.explicit_width(available_box).unwrap_or_else(|| {
            InlineMeasurement::content_box_width(children, &style, available_box)
        });
        let margin_width = box_width + style.margin_left + style.margin_right;
        Some((
            style.float,
            margin_width.min(available).max(MIN_LAYOUT_WIDTH),
        ))
    }
}

impl super::super::layout::HtmlLayoutRenderer {
    pub(super) fn render_floated_node(
        &mut self,
        node: &HtmlDocumentNode,
        side: CssFloat,
        float_width: f32,
        inherited: &CssStyle,
        details: super::super::types::DetailsContext,
        inline: &InlineFlowState,
    ) {
        let x = match side {
            CssFloat::Right => inline.x + (inline.width - float_width).max(0.0),
            CssFloat::Left | CssFloat::None => inline.x,
        };
        self.render_node(node, x, inline.y, float_width, inherited, details);
    }
}
