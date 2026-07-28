use super::super::constants::{CONTROL_HEIGHT, DEFAULT_INPUT_WIDTH, MIN_LAYOUT_WIDTH};
use super::super::document::attribute;
use super::super::style::CssStyle;

const CHECKBOX_INSET_SCALE: f32 = 4.0;
const CHECKBOX_CHECKMARK_LEFT_RATIO: f32 = 0.48;
const CHECKBOX_CHECKMARK_MIDDLE_X_RATIO: f32 = 0.08;
const CHECKBOX_CHECKMARK_MIDDLE_Y_RATIO: f32 = 0.36;
const CHECKBOX_CHECKMARK_RIGHT_RATIO: f32 = 0.55;
const CHECKBOX_CHECKMARK_TOP_RATIO: f32 = 0.4;
const CHECKBOX_MIN_INSET: f32 = 2.0;

pub(super) const CHECKBOX_STROKE: &str = "#8c959f";
pub(super) const CHECKBOX_CHECK: &str = "#16a34a";

#[derive(Clone, Copy)]
pub(super) struct InputGeometry {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

pub(super) fn input_geometry(x: f32, y: f32, width: f32, style: &CssStyle) -> InputGeometry {
    let available_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let width = style
        .explicit_width(available_width)
        .unwrap_or(available_width.min(DEFAULT_INPUT_WIDTH))
        .min(available_width);
    InputGeometry {
        x: x + style.margin_left,
        y: y + style.margin_top,
        width,
        height: style
            .height
            .map(|height| style.outer_height(height))
            .unwrap_or_else(|| style.outer_height(style.line_height).max(CONTROL_HEIGHT))
            .max(style.minimum_outer_height()),
    }
}

pub(super) fn checkbox_geometry(x: f32, y: f32, width: f32, style: &CssStyle) -> InputGeometry {
    let available_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let width = style
        .explicit_width(available_width)
        .unwrap_or(CONTROL_HEIGHT)
        .min(available_width);
    InputGeometry {
        x: x + style.margin_left,
        y: y + style.margin_top,
        width,
        height: style.height.unwrap_or(width).max(style.min_height),
    }
}

pub(super) fn checkbox_inset(dimension: f32) -> f32 {
    CHECKBOX_MIN_INSET.min(dimension / CHECKBOX_INSET_SCALE)
}

pub(super) fn is_checkbox(attributes: &[(String, String)]) -> bool {
    attribute(attributes, "type").is_some_and(|value| value.eq_ignore_ascii_case("checkbox"))
}

pub(super) const fn checkbox_check_ratio_left() -> f32 {
    CHECKBOX_CHECKMARK_LEFT_RATIO
}

pub(super) const fn checkbox_check_ratio_middle_x() -> f32 {
    CHECKBOX_CHECKMARK_MIDDLE_X_RATIO
}

pub(super) const fn checkbox_check_ratio_middle_y() -> f32 {
    CHECKBOX_CHECKMARK_MIDDLE_Y_RATIO
}

pub(super) const fn checkbox_check_ratio_right() -> f32 {
    CHECKBOX_CHECKMARK_RIGHT_RATIO
}

pub(super) const fn checkbox_check_ratio_top() -> f32 {
    CHECKBOX_CHECKMARK_TOP_RATIO
}
