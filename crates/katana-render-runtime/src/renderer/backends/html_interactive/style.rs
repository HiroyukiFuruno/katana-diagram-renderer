use super::constants::{
    DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT, FONT_WEIGHT_NORMAL, MONOSPACE_CHARACTER_WIDTH_FACTOR,
    TEXT_CHARACTER_WIDTH_FACTOR,
};
use super::document::attribute;
use crate::renderer::backends::html_css_rule::parse_declarations;
use taffy::style::{Display, FlexDirection, FlexWrap};

#[path = "style_declaration.rs"]
mod declaration;
#[path = "style_dimension_declaration.rs"]
mod dimension_declaration;
#[path = "style_border_declaration.rs"]
mod style_border_declaration;
#[path = "style_box.rs"]
mod style_box;
#[path = "style_box_declaration.rs"]
mod style_box_declaration;
#[path = "style_flex_declaration.rs"]
mod style_flex_declaration;
#[path = "style_shadow_declaration.rs"]
mod style_shadow_declaration;
#[path = "style_typography_declaration.rs"]
mod style_typography_declaration;
#[path = "style_tag_defaults.rs"]
mod tag_defaults;
#[path = "style_types.rs"]
mod types;
#[path = "style_value.rs"]
mod value;

pub(super) use types::{
    CssBoxShadow, CssBoxSizing, CssFloat, CssGridTrack, CssGridTrackBreadth, CssLength,
    CssOverflow, CssPosition, CssStyle, CssTextAlign, CssTextTransform, CssWhiteSpace,
};

const DEFAULT_STYLE_VIEWPORT_WIDTH: f32 = 1_024.0;
const DEFAULT_STYLE_VIEWPORT_HEIGHT: f32 = 768.0;

impl CssStyle {
    pub(super) fn browser_default() -> Self {
        Self::browser_default_for_viewport(
            DEFAULT_STYLE_VIEWPORT_WIDTH,
            DEFAULT_STYLE_VIEWPORT_HEIGHT,
        )
    }

    pub(super) fn browser_default_for_viewport(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            color: "#1f2328".to_string(),
            opacity: 1.0,
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            line_height_factor: Some(DEFAULT_LINE_HEIGHT / DEFAULT_FONT_SIZE),
            font_family: "Noto Sans, sans-serif".to_string(),
            font_weight: FONT_WEIGHT_NORMAL,
            display: Display::Block,
            inline_block: false,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::NoWrap,
            flex_shrink: 1.0,
            automatic_min_height: true,
            grid_template_columns: vec![CssGridTrack::Fraction(1.0)],
            viewport_width,
            viewport_height,
            ..<Self as Default>::default()
        }
    }

    #[cfg(test)]
    pub(super) fn from_attributes(attributes: &[(String, String)], inherited: &Self) -> Self {
        Self::from_element_attributes(None, attributes, inherited)
    }

    pub(super) fn from_element(
        tag: &str,
        attributes: &[(String, String)],
        inherited: &Self,
    ) -> Self {
        Self::from_element_attributes(Some(tag), attributes, inherited)
    }

    fn from_element_attributes(
        tag: Option<&str>,
        attributes: &[(String, String)],
        inherited: &Self,
    ) -> Self {
        let mut style = inherited.element_defaults();
        if let Some(tag) = tag {
            style.apply_tag_metrics(tag);
        }
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
            font_feature_settings: self.font_feature_settings.clone(),
            font_weight: self.font_weight,
            italic: self.italic,
            underline: self.underline,
            text_align: self.text_align,
            text_transform: self.text_transform,
            white_space: self.white_space,
            list_style_none: self.list_style_none,
            letter_spacing: self.letter_spacing,
            viewport_width: self.viewport_width,
            viewport_height: self.viewport_height,
            percentage_height_basis: self.percentage_height_basis,
            ..Self::browser_default()
        }
    }

    pub(super) fn inherited_text_style(&self) -> Self {
        self.element_defaults()
    }

    fn resolve_inherited_line_height(&mut self) {
        if let Some(factor) = self.line_height_factor {
            self.line_height = self.font_size * factor;
        }
    }

    pub(super) fn text_character_width_factor(&self) -> f32 {
        if self.font_family.to_ascii_lowercase().contains("mono") {
            MONOSPACE_CHARACTER_WIDTH_FACTOR
        } else {
            TEXT_CHARACTER_WIDTH_FACTOR
        }
    }

    pub(super) fn transformed_text<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        match self.text_transform {
            CssTextTransform::None => std::borrow::Cow::Borrowed(text),
            CssTextTransform::Uppercase => std::borrow::Cow::Owned(text.to_uppercase()),
            CssTextTransform::Lowercase => std::borrow::Cow::Owned(text.to_lowercase()),
            CssTextTransform::Capitalize => {
                let mut word_start = true;
                let mut transformed = String::with_capacity(text.len());
                for character in text.chars() {
                    if word_start && character.is_alphabetic() {
                        transformed.extend(character.to_uppercase());
                    } else {
                        transformed.push(character);
                    }
                    word_start = !character.is_alphanumeric();
                }
                std::borrow::Cow::Owned(transformed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::constants::LINE_HEIGHT_FACTOR;
    use super::{
        CssBoxSizing, CssGridTrack, CssLength, CssOverflow, CssStyle, CssTextAlign,
        DEFAULT_FONT_SIZE,
    };

    fn parsed_flow_style() -> CssStyle {
        CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "display:grid;gap:12px;grid-template-columns:repeat(3,1fr);flex-direction:column;flex-wrap:wrap;flex-grow:2;flex-shrink:0;align-items:center;justify-content:space-between".to_string(),
            )],
            &CssStyle::browser_default(),
        )
    }

    fn parsed_flex_style(value: &str) -> CssStyle {
        CssStyle::from_attributes(
            &[("style".to_string(), format!("flex:{value}"))],
            &CssStyle::browser_default(),
        )
    }

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
            "margin-top: 3px; margin-bottom: 4px; margin: var(--missing)".to_string(),
        )];
        let style = CssStyle::from_attributes(&attributes, &CssStyle::browser_default());

        assert_eq!(style.margin_top, 3.0);
        assert_eq!(style.margin_bottom, 4.0);
    }

    #[test]
    fn flow_properties_parse_into_typed_layout_values() {
        let style = parsed_flow_style();

        assert_eq!(style.display, taffy::style::Display::Grid);
        assert_eq!(style.gap, 12.0);
        assert_eq!(
            style.grid_template_columns,
            vec![CssGridTrack::Fraction(1.0); 3]
        );
        assert_eq!(style.flex_direction, taffy::style::FlexDirection::Column);
        assert_eq!(style.flex_wrap, taffy::style::FlexWrap::Wrap);
    }

    #[test]
    fn flow_alignment_and_flex_factors_parse_into_typed_values() {
        let style = parsed_flow_style();

        assert_eq!(style.flex_grow, 2.0);
        assert_eq!(style.flex_shrink, 0.0);
        assert_eq!(style.flex_basis, None);
        assert_eq!(style.align_items, Some(taffy::style::AlignItems::CENTER));
        assert_eq!(
            style.justify_content,
            Some(taffy::style::JustifyContent::SPACE_BETWEEN)
        );
    }

    #[test]
    fn flex_shorthand_sets_browser_compatible_grow_shrink_and_basis() {
        let one = parsed_flex_style("1");
        assert_eq!(one.flex_grow, 1.0);
        assert_eq!(one.flex_shrink, 1.0);
        assert_eq!(one.flex_basis, Some(CssLength::Percent(0.0)));

        let explicit = parsed_flex_style("2 0 25%");
        assert_eq!(explicit.flex_grow, 2.0);
        assert_eq!(explicit.flex_shrink, 0.0);
        assert_eq!(explicit.flex_basis, Some(CssLength::Percent(0.25)));
    }

    #[test]
    fn flex_shorthand_keywords_and_explicit_basis_use_browser_defaults() {
        let auto = parsed_flex_style("auto");
        assert_eq!(
            (auto.flex_grow, auto.flex_shrink, auto.flex_basis),
            (1.0, 1.0, None)
        );
        let none = parsed_flex_style("none");
        assert_eq!(
            (none.flex_grow, none.flex_shrink, none.flex_basis),
            (0.0, 0.0, None)
        );
        let initial = parsed_flex_style("initial");
        assert_eq!(
            (initial.flex_grow, initial.flex_shrink, initial.flex_basis),
            (0.0, 1.0, None)
        );

        let basis = CssStyle::from_attributes(
            &[("style".to_string(), "flex-basis:40px".to_string())],
            &CssStyle::browser_default(),
        );
        assert_eq!(basis.flex_basis, Some(CssLength::Px(40.0)));
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
    fn assigned_flow_width_replaces_pre_layout_width_constraints() {
        let mut assigned_outer = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "box-sizing: content-box; min-width: 60px; max-width: 80px; padding: 10px"
                    .to_string(),
            )],
            &CssStyle::browser_default(),
        );
        assigned_outer.assign_outer_width(120.0);
        assert_eq!(assigned_outer.box_width(200.0), 120.0);
        assert_eq!(assigned_outer.width, Some(CssLength::Px(100.0)));
        assert_eq!(assigned_outer.min_width, None);
        assert_eq!(assigned_outer.max_width, None);
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
    fn monospace_families_use_their_advance_width_for_layout_measurement() {
        let proportional = CssStyle::browser_default();
        let monospace = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                "font-family: IBM Plex Mono, monospace".to_string(),
            )],
            &proportional,
        );

        assert_eq!(proportional.text_character_width_factor(), 0.55);
        assert_eq!(monospace.text_character_width_factor(), 0.6);
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
        assert_eq!(content_box.border_radius, CssLength::Px(6.0));
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
