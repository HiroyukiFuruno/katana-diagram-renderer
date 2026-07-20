use super::constants::{
    CONTROL_HEIGHT, DEFAULT_INPUT_WIDTH, INPUT_TEXT_LEFT_PADDING, MIN_LAYOUT_WIDTH,
};
use super::control_style::{
    button_style, button_width, details_is_open, input_style, summary_height,
    visible_details_children,
};
use super::document::input_initial_value;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{
    ControlLayout, DetailsContext, ElementRenderContext, HitTargetKind, LayoutContext,
};

impl HtmlLayoutRenderer {
    pub(super) fn render_button(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let start = layout.y + layout.style.margin_top;
        let x = layout.x + layout.style.margin_left;
        let available_width = (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH);
        let button_width = button_width(element.children, available_width, layout.style);
        let height = layout
            .style
            .height
            .unwrap_or(CONTROL_HEIGHT)
            .max(layout.style.min_height);
        let style = button_style(layout.style);
        self.paint_button(
            element,
            ControlLayout {
                x,
                y: start,
                width: button_width,
                height,
                style: &style,
            },
        );
        start + height + style.margin_bottom
    }

    pub(super) fn render_input(
        &mut self,
        node_id: u64,
        attributes: &[(String, String)],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let start = y + style.margin_top;
        let x = x + style.margin_left;
        let available_width =
            (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let input_width = style
            .explicit_width(available_width)
            .unwrap_or(available_width.min(DEFAULT_INPUT_WIDTH))
            .min(available_width);
        let height = style.height.unwrap_or(CONTROL_HEIGHT).max(style.min_height);
        let value = self.input_value(node_id, attributes);
        let style = input_style(style, self.focused_input == Some(node_id));
        self.paint_box(x, start, input_width, height, &style);
        self.paint_control_text(&value, x + INPUT_TEXT_LEFT_PADDING, start, height, &style);
        self.push_target(node_id, x, start, input_width, height, HitTargetKind::Input);
        start + height + style.margin_bottom
    }

    pub(super) fn render_details(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let start = layout.y + layout.style.margin_top;
        let x = layout.x + layout.style.margin_left;
        let width = (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH);
        let details_open = details_is_open(element.attributes);
        let content = visible_details_children(element.attributes, element.children);
        let inner_x = x + layout.style.padding_left;
        let inner_width =
            (width - layout.style.padding_left - layout.style.padding_right).max(MIN_LAYOUT_WIDTH);
        let box_start = self.svg.len();
        let bottom = self.render_nodes(
            &content,
            inner_x,
            start + layout.style.padding_top,
            inner_width,
            layout.style,
            DetailsContext::from_open_state(element.node_id, details_open),
        );
        let height = (bottom - start + layout.style.padding_bottom).max(CONTROL_HEIGHT);
        self.insert_box(box_start, x, start, width, height, layout.style);
        start + height + layout.style.margin_bottom
    }

    pub(super) fn render_summary(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let control = summary_layout(layout);
        self.paint_summary(element.children, control, layout.details.open);
        self.push_summary_target(
            element.node_id,
            layout.details.node_id,
            control.x,
            control.y,
            control.width,
            control.height,
        );
        control.y + control.height + control.style.margin_bottom
    }

    fn push_summary_target(
        &mut self,
        node_id: u64,
        details_node_id: Option<u64>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        if let Some(details_node_id) = details_node_id {
            self.push_target(
                node_id,
                x,
                y,
                width,
                height,
                HitTargetKind::Summary { details_node_id },
            );
        }
    }

    fn input_value(&mut self, node_id: u64, attributes: &[(String, String)]) -> String {
        self.input_values
            .entry(node_id)
            .or_insert_with(|| input_initial_value(attributes))
            .clone()
    }
}

fn summary_layout(layout: LayoutContext<'_>) -> ControlLayout<'_> {
    ControlLayout {
        x: layout.x + layout.style.margin_left,
        y: layout.y + layout.style.margin_top,
        width: (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH),
        height: summary_height(layout.style),
        style: layout.style,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocumentNode;
    use std::collections::HashMap;

    #[test]
    fn input_layout_uses_attribute_value_when_session_seed_is_absent() -> Result<(), String> {
        let nodes = vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "input".to_string(),
            attributes: vec![("value".to_string(), "fallback value".to_string())],
            children: Vec::new(),
        }];
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;

        assert!(layout.svg.contains("fallback value"));
        Ok(())
    }
}
