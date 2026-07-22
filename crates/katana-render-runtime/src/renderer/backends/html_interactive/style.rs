use super::constants::{DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT};
use super::document::attribute;
use crate::renderer::backends::html_css_rule::parse_declarations;
use taffy::style::{AlignItems, Display, FlexDirection, FlexWrap, JustifyContent};

#[path = "style_declaration.rs"]
mod declaration;
#[path = "style_box.rs"]
mod style_box;
#[path = "style_box_declaration.rs"]
mod style_box_declaration;
#[path = "style_typography_declaration.rs"]
mod style_typography_declaration;
#[path = "style_tag_defaults.rs"]
mod tag_defaults;
#[path = "style_value.rs"]
mod value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CssLength {
    Px(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CssGridTrack {
    Length(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    MinContent,
    MaxContent,
    MinMax {
        min: CssGridTrackBreadth,
        max: CssGridTrackBreadth,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum CssGridTrackBreadth {
    Length(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    MinContent,
    MaxContent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum CssBoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum CssOverflow {
    #[default]
    Visible,
    Clip,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum CssTextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Default)]
pub(super) struct CssStyle {
    pub(super) color: String,
    pub(super) background: Option<String>,
    pub(super) border: Option<String>,
    pub(super) border_width: f32,
    pub(super) border_radius: f32,
    pub(super) box_sizing: CssBoxSizing,
    pub(super) overflow: CssOverflow,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    pub(super) line_height_factor: Option<f32>,
    pub(super) font_family: String,
    pub(super) italic: bool,
    pub(super) text_align: CssTextAlign,
    pub(super) letter_spacing: f32,
    pub(super) padding_top: f32,
    pub(super) padding_right: f32,
    pub(super) padding_bottom: f32,
    pub(super) padding_left: f32,
    pub(super) margin_top: f32,
    pub(super) margin_right: f32,
    pub(super) margin_bottom: f32,
    pub(super) margin_left: f32,
    pub(super) min_height: f32,
    pub(super) width: Option<CssLength>,
    pub(super) max_width: Option<CssLength>,
    pub(super) height: Option<f32>,
    pub(super) bold: bool,
    pub(super) underline: bool,
    pub(super) display: Display,
    pub(super) inline_block: bool,
    pub(super) gap: f32,
    pub(super) flex_direction: FlexDirection,
    pub(super) flex_wrap: FlexWrap,
    pub(super) flex_grow: f32,
    pub(super) flex_shrink: f32,
    pub(super) align_items: Option<AlignItems>,
    pub(super) justify_content: Option<JustifyContent>,
    pub(super) grid_template_columns: Vec<CssGridTrack>,
    pub(super) explicit_color: bool,
    pub(super) explicit_background: bool,
}

impl CssStyle {
    pub(super) fn browser_default() -> Self {
        Self {
            color: "#1f2328".to_string(),
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            line_height_factor: Some(DEFAULT_LINE_HEIGHT / DEFAULT_FONT_SIZE),
            font_family: "Noto Sans, sans-serif".to_string(),
            display: Display::Block,
            inline_block: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_shrink: 1.0,
            grid_template_columns: vec![CssGridTrack::Fraction(1.0)],
            ..<Self as Default>::default()
        }
    }

    pub(super) fn from_attributes(attributes: &[(String, String)], inherited: &Self) -> Self {
        let mut style = inherited.element_defaults();
        if attribute(attributes, "hidden").is_some() {
            style.display = Display::None;
        }
        let Some(source) = attribute(attributes, "style") else {
            return style;
        };
        let declarations = parse_declarations(source);
        let mut line_heights = Vec::new();
        for declaration in &declarations {
            if declaration.name.eq_ignore_ascii_case("line-height") {
                line_heights.push(declaration.value.as_str());
            } else {
                style.apply(&declaration.name, &declaration.value);
            }
        }
        style.resolve_inherited_line_height();
        for line_height in line_heights {
            style.apply_line_height(line_height);
        }
        style
    }

    fn element_defaults(&self) -> Self {
        Self {
            color: self.color.clone(),
            font_size: self.font_size,
            line_height: self.line_height,
            line_height_factor: self.line_height_factor,
            font_family: self.font_family.clone(),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            text_align: self.text_align,
            letter_spacing: self.letter_spacing,
            ..Self::browser_default()
        }
    }

    pub(super) fn for_tag(mut self, tag: &str) -> Self {
        self.apply_tag_metrics(tag);
        self.resolve_inherited_line_height();
        self
    }

    fn resolve_inherited_line_height(&mut self) {
        if let Some(factor) = self.line_height_factor {
            self.line_height = self.font_size * factor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::constants::LINE_HEIGHT_FACTOR;
    use super::{
        CssBoxSizing, CssGridTrack, CssOverflow, CssStyle, CssTextAlign, DEFAULT_FONT_SIZE,
    };

    #[test]
    fn box_shorthands_expand_and_longhands_override_individual_edges() {
        let attributes = vec![(
            "style".to_string(),
            "padding: 1px 2px 3px 4px; margin: -5px 6px 7px; padding-left: 9px; margin-right: 8px"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left,
            ],
            [1.0, 2.0, 3.0, 9.0]
        );
        assert_eq!(
            [
                style.margin_top,
                style.margin_right,
                style.margin_bottom,
                style.margin_left,
            ],
            [-5.0, 8.0, 7.0, 6.0]
        );
    }

    #[test]
    fn invalid_box_shorthand_does_not_erase_existing_edges() {
        let attributes = vec![(
            "style".to_string(),
            "margin-top: 3px; margin-bottom: 4px; margin: auto auto".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.margin_top, 3.0);
        assert_eq!(style.margin_bottom, 4.0);
    }

    #[test]
    fn flow_properties_parse_into_typed_layout_values() {
        let attributes = vec![(
            "style".to_string(),
            "display: grid; gap: 12px; grid-template-columns: repeat(3, 1fr); flex-direction: column; flex-wrap: wrap; flex-grow: 2; flex-shrink: 0; align-items: center; justify-content: space-between"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.display, taffy::style::Display::Grid);
        assert_eq!(style.gap, 12.0);
        assert_eq!(
            style.grid_template_columns,
            vec![CssGridTrack::Fraction(1.0); 3]
        );
        assert_eq!(style.flex_direction, taffy::style::FlexDirection::Column);
        assert_eq!(style.flex_wrap, taffy::style::FlexWrap::Wrap);
        assert_eq!(style.flex_grow, 2.0);
        assert_eq!(style.flex_shrink, 0.0);
        assert_eq!(style.align_items, Some(taffy::style::AlignItems::CENTER));
        assert_eq!(
            style.justify_content,
            Some(taffy::style::JustifyContent::SPACE_BETWEEN)
        );
    }

    #[test]
    fn percentage_width_and_max_width_resolve_against_available_space() {
        let attributes = vec![(
            "style".to_string(),
            "width: 75%; max-width: 200px".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.explicit_width(400.0), Some(200.0));
        assert_eq!(style.explicit_width(200.0), Some(150.0));

        let max_only = vec![("style".to_string(), "max-width: 120px".to_string())];
        let max_only = CssStyle::from_attributes(&max_only, &CssStyle::browser_default());
        assert_eq!(max_only.box_width(400.0), 120.0);
    }

    #[test]
    fn assigned_flow_width_consumes_width_and_its_max_constraint() {
        let attributes = vec![(
            "style".to_string(),
            "width: 50%; max-width: 120px".to_string(),
        )];
        let mut assigned = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assigned.consume_assigned_flow_width();
        assert_eq!(assigned.box_width(120.0), 120.0);

        let max_only = vec![("style".to_string(), "max-width: 80px".to_string())];
        let mut max_only = CssStyle::from_attributes(&max_only, &CssStyle::browser_default());
        max_only.consume_assigned_flow_width();
        assert_eq!(max_only.box_width(120.0), 80.0);
    }

    #[test]
    fn relative_font_and_box_lengths_resolve_without_unitless_line_height_bug() {
        let attributes = vec![(
            "style".to_string(),
            "font-size: 1.5rem; line-height: 1.25; padding: 0.5em; margin-top: -1rem".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.font_size, 24.0);
        assert_eq!(style.line_height, 30.0);
        assert_eq!(style.padding_top, 12.0);
        assert_eq!(style.margin_top, -16.0);
    }

    #[test]
    fn unitless_line_height_resolves_after_font_size_and_scales_when_inherited() {
        let before_font_size = vec![(
            "style".to_string(),
            "line-height: 1.5; font-size: 20px".to_string(),
        )];
        let parent = CssStyle::from_attributes(&before_font_size, &CssStyle::browser_default());
        assert_eq!(parent.line_height, 30.0);

        let child_attributes = vec![("style".to_string(), "font-size: 24px".to_string())];
        let child = CssStyle::from_attributes(&child_attributes, &parent);
        assert_eq!(child.line_height, 36.0);

        let fixed_parent = vec![(
            "style".to_string(),
            "line-height: 30px; font-size: 20px".to_string(),
        )];
        let fixed_parent = CssStyle::from_attributes(&fixed_parent, &CssStyle::browser_default());
        let fixed_child = CssStyle::from_attributes(&child_attributes, &fixed_parent);
        assert_eq!(fixed_child.line_height, 30.0);
    }

    #[test]
    fn invalid_line_height_does_not_erase_the_inherited_value() {
        let attributes = vec![("style".to_string(), "line-height: invalid".to_string())];
        let inherited = CssStyle::browser_default();
        let style = CssStyle::from_attributes(&attributes, &inherited);

        assert_eq!(style.line_height, inherited.line_height);
        assert_eq!(style.line_height_factor, inherited.line_height_factor);
    }

    #[test]
    fn hidden_attribute_maps_to_display_none() {
        let attributes = vec![("hidden".to_string(), String::new())];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.display, taffy::style::Display::None);
    }

    #[test]
    fn padding_longhands_and_invalid_extensions_preserve_each_edge() {
        let attributes = vec![(
            "style".to_string(),
            "padding: 1px; padding-top: 2px; padding-right: 3px; padding-bottom: 4px; padding-inline: 9px; margin-inline: 9px; padding: 1px 2px 3px 4px 5px"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left,
            ],
            [2.0, 3.0, 4.0, 1.0]
        );
    }

    #[test]
    fn alternate_relative_and_grid_values_resolve_without_fallbacks() {
        let attributes = vec![(
            "style".to_string(),
            "padding: 2px 3px; font-size: 125%; line-height: 150%; grid-template-columns: 1fr 2fr 1fr"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assert_eq!(style.padding_top, 2.0);
        assert_eq!(style.padding_right, 3.0);
        assert_eq!(style.font_size, 20.0);
        assert_eq!(style.line_height, 30.0);
        assert_eq!(
            style.grid_template_columns,
            vec![
                CssGridTrack::Fraction(1.0),
                CssGridTrack::Fraction(2.0),
                CssGridTrack::Fraction(1.0),
            ]
        );

        let normal = vec![("style".to_string(), "line-height: normal".to_string())];
        let normal = CssStyle::from_attributes(&normal, &CssStyle::browser_default());
        assert_eq!(normal.line_height, DEFAULT_FONT_SIZE * LINE_HEIGHT_FACTOR);

        let em = vec![("style".to_string(), "line-height: 2em".to_string())];
        let em = CssStyle::from_attributes(&em, &CssStyle::browser_default());
        assert_eq!(em.line_height, DEFAULT_FONT_SIZE * 2.0);
    }

    #[test]
    fn grid_tracks_preserve_declared_sizes() {
        let attributes = vec![(
            "style".to_string(),
            "display:grid; grid-template-columns: 100px max-content 2fr 25%".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assert_eq!(
            style.grid_template_columns,
            vec![
                CssGridTrack::Length(100.0),
                CssGridTrack::MaxContent,
                CssGridTrack::Fraction(2.0),
                CssGridTrack::Percent(0.25),
            ]
        );
    }

    #[test]
    fn grid_repeat_expands_each_declared_track() {
        let repeat = vec![(
            "style".to_string(),
            "grid-template-columns: repeat(2, 80px 1fr)".to_string(),
        )];
        let repeat = CssStyle::from_attributes(&repeat, &CssStyle::browser_default());
        assert_eq!(
            repeat.grid_template_columns,
            vec![
                CssGridTrack::Length(80.0),
                CssGridTrack::Fraction(1.0),
                CssGridTrack::Length(80.0),
                CssGridTrack::Fraction(1.0),
            ]
        );
    }

    #[test]
    fn invalid_grid_tracks_preserve_the_inherited_template() {
        let attributes = vec![(
            "style".to_string(),
            "grid-template-columns: minmax(broken)".to_string(),
        )];
        let inherited = CssStyle::browser_default();
        let style = CssStyle::from_attributes(&attributes, &inherited);

        assert_eq!(style.grid_template_columns, inherited.grid_template_columns);
    }

    #[test]
    fn typed_content_box_properties_preserve_browser_box_model() {
        let attributes = vec![(
            "style".to_string(),
            "width: 100px; padding: 10px; border: 2px solid #123456; border-radius: 6px; overflow: hidden"
                .to_string(),
        )];
        let content_box = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(content_box.box_sizing, CssBoxSizing::ContentBox);
        assert_eq!(content_box.explicit_width(300.0), Some(124.0));
        assert_eq!(content_box.content_width(124.0), 100.0);
        assert_eq!(content_box.border_width, 2.0);
        assert_eq!(content_box.border_radius, 6.0);
        assert_eq!(content_box.overflow, CssOverflow::Clip);
    }

    #[test]
    fn typed_typography_properties_preserve_browser_values() {
        let attributes = vec![(
            "style".to_string(),
            "font-family: Inter, sans-serif; font-style: italic; text-align: center; letter-spacing: 2px"
                .to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.font_family, "Inter, sans-serif");
        assert!(style.italic);
        assert_eq!(style.text_align, CssTextAlign::Center);
        assert_eq!(style.letter_spacing, 2.0);
    }

    #[test]
    fn typed_border_box_dimensions_preserve_browser_box_model() {
        let attributes = vec![(
            "style".to_string(),
            "box-sizing: border-box; width: 100px; padding: 10px; border-width: 2px".to_string(),
        )];
        let border_box = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());
        assert_eq!(border_box.box_sizing, CssBoxSizing::BorderBox);
        assert_eq!(border_box.explicit_width(300.0), Some(100.0));
        assert_eq!(border_box.content_width(100.0), 76.0);
    }
}
