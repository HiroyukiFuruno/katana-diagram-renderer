use super::constants::{
    CONTROL_HEIGHT, DEFAULT_INPUT_WIDTH, INPUT_TEXT_LEFT_PADDING, MIN_LAYOUT_WIDTH,
};
use super::control_style::input_style;
use super::document::input_initial_value;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::HitTargetKind;

#[derive(Clone, Copy)]
struct InputGeometry {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl HtmlLayoutRenderer {
    pub(super) fn render_input(
        &mut self,
        node_id: u64,
        attributes: &[(String, String)],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let geometry = input_geometry(x, y, width, style);
        let value = self.input_value(node_id, attributes);
        let style = input_style(style, self.focused_input == Some(node_id));
        self.paint_input_control(node_id, &value, geometry, &style);
        geometry.y + geometry.height + style.margin_bottom
    }

    fn paint_input_control(
        &mut self,
        node_id: u64,
        value: &str,
        geometry: InputGeometry,
        style: &CssStyle,
    ) {
        self.paint_box(
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            style,
        );
        self.paint_input_value(value, geometry, style);
        self.push_target(
            node_id,
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
            HitTargetKind::Input,
        );
    }

    fn paint_input_value(&mut self, value: &str, geometry: InputGeometry, style: &CssStyle) {
        self.paint_control_text(
            value,
            geometry.x + INPUT_TEXT_LEFT_PADDING,
            (geometry.width - INPUT_TEXT_LEFT_PADDING * 2.0).max(0.0),
            geometry.y,
            geometry.height,
            style,
        );
    }

    fn input_value(&mut self, node_id: u64, attributes: &[(String, String)]) -> String {
        self.input_values
            .entry(node_id)
            .or_insert_with(|| input_initial_value(attributes))
            .clone()
    }
}

fn input_geometry(x: f32, y: f32, width: f32, style: &CssStyle) -> InputGeometry {
    let available_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let width = style
        .explicit_width(available_width)
        .unwrap_or(available_width.min(DEFAULT_INPUT_WIDTH))
        .min(available_width);
    InputGeometry {
        x: x + style.margin_left,
        y: y + style.margin_top,
        width,
        height: style.height.unwrap_or(CONTROL_HEIGHT).max(style.min_height),
    }
}
