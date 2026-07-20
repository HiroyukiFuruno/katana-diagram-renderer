use super::super::constants::{BOLD_FONT_WEIGHT_MINIMUM, DEFAULT_FONT_SIZE, LINE_HEIGHT_FACTOR};
use super::CssLength;

const CSS_BOX_SIDE_COUNT: usize = 4;

impl CssLength {
    pub(super) fn parse(value: &str, em_base: f32) -> Option<Self> {
        let value = value.trim();
        if let Some(percent) = value.strip_suffix('%') {
            return css_number(percent).map(|value| Self::Percent(value / 100.0));
        }
        css_relative_px(value, em_base, false).map(Self::Px)
    }

    pub(super) fn resolve(self, available: f32) -> f32 {
        match self {
            Self::Px(value) => value,
            Self::Percent(value) => available * value,
        }
    }
}

pub(super) fn box_sides(
    value: &str,
    em_base: f32,
    signed: bool,
) -> Option<[f32; CSS_BOX_SIDE_COUNT]> {
    let values = value
        .split_whitespace()
        .map(|value| css_relative_px(value, em_base, signed))
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [all] => Some([*all; CSS_BOX_SIDE_COUNT]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

pub(super) fn css_relative_px(value: &str, em_base: f32, signed: bool) -> Option<f32> {
    let value = value.trim();
    let parsed = if let Some(value) = value.strip_suffix("rem") {
        css_scalar(value).map(|value| value * DEFAULT_FONT_SIZE)
    } else if let Some(value) = value.strip_suffix("em") {
        css_scalar(value).map(|value| value * em_base)
    } else {
        css_scalar(value.strip_suffix("px").unwrap_or(value))
    }?;
    (signed || parsed >= 0.0).then_some(parsed)
}

pub(super) fn css_font_size(value: &str, inherited: f32) -> Option<f32> {
    if let Some(value) = value.trim().strip_suffix('%') {
        return css_number(value).map(|value| inherited * value / 100.0);
    }
    css_relative_px(value, inherited, false)
}

pub(super) fn css_line_height(value: &str, font_size: f32) -> Option<f32> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("normal") {
        return Some(font_size * LINE_HEIGHT_FACTOR);
    }
    if let Some(factor) = css_number(value) {
        return Some(factor * font_size);
    }
    if let Some(value) = value.strip_suffix('%') {
        return css_number(value).map(|value| font_size * value / 100.0);
    }
    css_relative_px(value, font_size, false)
}

pub(super) fn css_number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(super) fn grid_column_count(value: &str) -> Option<usize> {
    let value = value.trim();
    if let Some(arguments) = value
        .strip_prefix("repeat(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return arguments
            .split_once(',')
            .and_then(|(count, _)| count.trim().parse::<usize>().ok())
            .filter(|count| *count > 0);
    }
    let count = value.split_whitespace().count();
    (count > 0).then_some(count)
}

pub(super) fn is_bold(value: &str) -> bool {
    value.eq_ignore_ascii_case("bold")
        || value
            .parse::<u16>()
            .is_ok_and(|weight| weight >= BOLD_FONT_WEIGHT_MINIMUM)
}

fn css_scalar(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}
