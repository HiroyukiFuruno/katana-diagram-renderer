use super::super::constants::{FONT_WEIGHT_BOLD, FONT_WEIGHT_NORMAL, LINE_HEIGHT_FACTOR};
use super::CssLength;

#[path = "style_grid_value.rs"]
mod grid;
#[path = "style_math_value.rs"]
mod math;

pub(super) use grid::grid_tracks;
pub(super) use math::{css_relative_px, css_resolved_px, split_top_level_whitespace};

const CSS_BOX_SIDE_COUNT: usize = 4;
const FONT_WEIGHT_THIN: u16 = 100;
const FONT_WEIGHT_BLACK: u16 = 900;
const FONT_WEIGHT_MAX: u16 = 1_000;
const BOLDER_NORMAL_CEILING: u16 = 349;
const BOLDER_BOLD_CEILING: u16 = 549;
const LIGHTER_THIN_CEILING: u16 = 549;
const LIGHTER_NORMAL_CEILING: u16 = 749;

impl CssLength {
    pub(super) fn parse(
        value: &str,
        em_base: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<Self> {
        let value = value.trim();
        if let Some(percent) = value.strip_suffix('%') {
            return css_number(percent).map(|value| Self::Percent(value / 100.0));
        }
        css_resolved_px(value, em_base, viewport_width, viewport_height, false).map(Self::Px)
    }

    pub(in super::super) fn resolve(self, available: f32) -> f32 {
        match self {
            Self::Px(value) => value,
            Self::Percent(value) => available * value,
        }
    }
}

pub(super) fn box_sides(
    value: &str,
    em_base: f32,
    viewport_width: f32,
    viewport_height: f32,
    signed: bool,
) -> Option<[f32; CSS_BOX_SIDE_COUNT]> {
    let values = split_top_level_whitespace(value)
        .into_iter()
        .map(|value| css_resolved_px(value, em_base, viewport_width, viewport_height, signed))
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [all] => Some([*all; CSS_BOX_SIDE_COUNT]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

pub(super) fn css_font_size(
    value: &str,
    inherited: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<f32> {
    if let Some(value) = value.trim().strip_suffix('%') {
        return css_number(value).map(|value| inherited * value / 100.0);
    }
    css_resolved_px(value, inherited, viewport_width, viewport_height, false)
}

pub(super) fn css_line_height(
    value: &str,
    font_size: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(f32, Option<f32>)> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("normal") {
        return Some((font_size * LINE_HEIGHT_FACTOR, Some(LINE_HEIGHT_FACTOR)));
    }
    if let Some(factor) = css_number(value) {
        return Some((factor * font_size, Some(factor)));
    }
    if let Some(value) = value.strip_suffix('%') {
        return css_number(value).map(|value| (font_size * value / 100.0, None));
    }
    css_resolved_px(value, font_size, viewport_width, viewport_height, false)
        .map(|value| (value, None))
}

pub(super) fn css_number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(super) fn css_font_weight(value: &str, inherited: u16) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FONT_WEIGHT_NORMAL),
        "bold" => Some(FONT_WEIGHT_BOLD),
        "bolder" => Some(if inherited <= BOLDER_NORMAL_CEILING {
            FONT_WEIGHT_NORMAL
        } else if inherited <= BOLDER_BOLD_CEILING {
            FONT_WEIGHT_BOLD
        } else {
            FONT_WEIGHT_BLACK
        }),
        "lighter" => Some(if inherited <= LIGHTER_THIN_CEILING {
            FONT_WEIGHT_THIN
        } else if inherited <= LIGHTER_NORMAL_CEILING {
            FONT_WEIGHT_NORMAL
        } else {
            FONT_WEIGHT_BOLD
        }),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=FONT_WEIGHT_MAX).contains(weight)),
    }
}

#[cfg(test)]
mod tests {
    use super::{CssLength, box_sides, css_font_weight, css_relative_px, css_resolved_px};

    #[test]
    fn box_sides_accepts_all_supported_notations() {
        assert_eq!(box_sides("3px", 16.0, 1000.0, 500.0, false), Some([3.0; 4]));
        assert_eq!(
            box_sides("1px 2px", 16.0, 1000.0, 500.0, false),
            Some([1.0, 2.0, 1.0, 2.0])
        );
        assert_eq!(
            box_sides("1px 2px 3px", 16.0, 1000.0, 500.0, false),
            Some([1.0, 2.0, 3.0, 2.0])
        );
        assert_eq!(
            box_sides("1px 2px 3px 4px", 16.0, 1000.0, 500.0, false),
            Some([1.0, 2.0, 3.0, 4.0])
        );
        assert!(box_sides("1px 2px 3px 4px 5px", 16.0, 1000.0, 500.0, false).is_none());
    }

    #[test]
    fn font_weight_accepts_css_keywords() {
        assert_eq!(css_font_weight("normal", 600), Some(400));
        assert_eq!(css_font_weight("bold", 400), Some(700));
        assert_eq!(css_font_weight("bolder", 300), Some(400));
        assert_eq!(css_font_weight("bolder", 400), Some(700));
        assert_eq!(css_font_weight("bolder", 800), Some(900));
        assert_eq!(css_font_weight("lighter", 400), Some(100));
        assert_eq!(css_font_weight("lighter", 700), Some(400));
        assert_eq!(css_font_weight("lighter", 800), Some(700));
    }

    #[test]
    fn font_weight_accepts_only_the_numeric_css_range() {
        assert_eq!(css_font_weight("600", 400), Some(600));
        assert_eq!(css_font_weight("0", 400), None);
        assert_eq!(css_font_weight("1001", 400), None);
    }

    #[test]
    fn relative_px_parses_units_and_signing_rules() {
        assert_eq!(css_relative_px("10px", 16.0, false), Some(10.0));
        assert_eq!(css_relative_px("1.5rem", 16.0, false), Some(24.0));
        assert_eq!(css_relative_px("2em", 16.0, false), Some(32.0));
        assert_eq!(css_relative_px("-5px", 16.0, false), None);
        assert_eq!(css_relative_px("-5px", 16.0, true), Some(-5.0));
        assert_eq!(css_length_percent("40%"), Some(CssLength::Percent(0.4)));
    }

    #[test]
    fn parse_percent_length_then_resolve() {
        assert_eq!(
            CssLength::parse("12.5%", 10.0, 1000.0, 500.0).map(|size| size.resolve(160.0)),
            Some(20.0)
        );
    }

    fn css_length_percent(value: &str) -> Option<CssLength> {
        CssLength::parse(value, 16.0, 1000.0, 500.0)
    }

    #[test]
    fn viewport_units_and_nested_math_resolve_against_the_page_viewport() {
        assert_eq!(
            css_resolved_px("4.6vw", 16.0, 1440.0, 900.0, false),
            Some(66.24)
        );
        assert_eq!(
            css_resolved_px("100vh", 16.0, 1440.0, 900.0, false),
            Some(900.0)
        );
        assert_eq!(
            css_resolved_px("10vmin", 16.0, 1440.0, 900.0, false),
            Some(90.0)
        );
        assert_eq!(
            css_resolved_px("10vmax", 16.0, 1440.0, 900.0, false),
            Some(144.0)
        );
    }

    #[test]
    fn nested_css_math_resolves_and_rejects_invalid_arity() {
        assert_eq!(
            css_resolved_px(
                "clamp(38px, min(4.6vw, 70px), 64px)",
                16.0,
                1440.0,
                900.0,
                false
            ),
            Some(64.0)
        );
        assert_eq!(
            css_resolved_px("max(12px, 2vw)", 16.0, 1000.0, 500.0, false),
            Some(20.0)
        );
        assert!(css_resolved_px("clamp(1px, 2px)", 16.0, 1000.0, 500.0, false).is_none());
    }
}
