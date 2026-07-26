use super::value::{css_font_size, css_font_weight, css_resolved_px};
use super::{CssStyle, CssTextAlign, CssTextTransform, CssWhiteSpace};

impl CssStyle {
    pub(super) fn apply_typography_property(&mut self, name: &str, value: &str) -> bool {
        match name.to_ascii_lowercase().as_str() {
            "font-size" => self.apply_font_size(value),
            "font-weight" => self.apply_font_weight(value),
            "font-family" => self.apply_font_family(value),
            "font-feature-settings" => self.apply_font_feature_settings(value),
            "font-style" => self.apply_font_style(value),
            "text-decoration" => self.underline = value.contains("underline"),
            "text-align" => self.apply_text_align(value),
            "text-transform" => self.apply_text_transform(value),
            "white-space" => self.apply_white_space(value),
            "letter-spacing" => self.apply_letter_spacing(value),
            _ => return false,
        }
        true
    }

    fn apply_font_size(&mut self, value: &str) {
        self.font_size = css_font_size(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        )
        .unwrap_or(self.font_size);
    }

    fn apply_font_weight(&mut self, value: &str) {
        self.font_weight = css_font_weight(value, self.font_weight).unwrap_or(self.font_weight);
    }

    fn apply_font_style(&mut self, value: &str) {
        self.italic = matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "italic" | "oblique"
        );
    }

    fn apply_font_family(&mut self, value: &str) {
        let family = value.trim();
        if !family.is_empty() {
            self.font_family = family.to_string();
        }
    }

    fn apply_font_feature_settings(&mut self, value: &str) {
        let settings = value.trim();
        self.font_feature_settings =
            if settings.is_empty() || settings.eq_ignore_ascii_case("normal") {
                None
            } else {
                Some(settings.to_string())
            };
    }

    fn apply_text_align(&mut self, value: &str) {
        self.text_align = match value.trim().to_ascii_lowercase().as_str() {
            "center" => CssTextAlign::Center,
            "right" | "end" => CssTextAlign::End,
            "left" | "start" => CssTextAlign::Start,
            _ => self.text_align,
        };
    }

    fn apply_text_transform(&mut self, value: &str) {
        self.text_transform = match value.trim().to_ascii_lowercase().as_str() {
            "none" => CssTextTransform::None,
            "uppercase" => CssTextTransform::Uppercase,
            "lowercase" => CssTextTransform::Lowercase,
            "capitalize" => CssTextTransform::Capitalize,
            _ => self.text_transform,
        };
    }

    fn apply_white_space(&mut self, value: &str) {
        self.white_space = match value.trim().to_ascii_lowercase().as_str() {
            "normal" => CssWhiteSpace::Normal,
            "nowrap" => CssWhiteSpace::NoWrap,
            _ => self.white_space,
        };
    }

    fn apply_letter_spacing(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("normal") {
            self.letter_spacing = 0.0;
        } else {
            self.letter_spacing = css_resolved_px(
                value,
                self.font_size,
                self.viewport_width,
                self.viewport_height,
                true,
            )
            .unwrap_or(self.letter_spacing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, CssTextAlign, CssTextTransform, CssWhiteSpace};

    #[test]
    fn typography_dispatches_known_properties_and_rejects_unknown() {
        let mut style = CssStyle::browser_default();

        assert!(style.apply_typography_property("font-size", "2rem"));
        assert!(style.apply_typography_property("font-weight", "700"));
        assert_eq!(style.font_weight, 700);
        assert!(style.apply_typography_property("font-style", "italic"));
        assert!(style.italic);

        assert!(!style.apply_typography_property("line-height", "160%"));
    }

    #[test]
    fn numeric_and_relative_font_weights_preserve_css_weight_values() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("font-weight", "600");
        assert_eq!(style.font_weight, 600);
        style.apply_typography_property("font-weight", "lighter");
        assert_eq!(style.font_weight, 400);
        style.apply_typography_property("font-weight", "bolder");
        assert_eq!(style.font_weight, 700);
        style.apply_typography_property("font-weight", "normal");
        assert_eq!(style.font_weight, 400);
    }

    #[test]
    fn font_family_keeps_current_if_empty() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans, sans-serif".to_string();

        style.apply_typography_property("font-family", "  ");

        assert_eq!(style.font_family, "Noto Sans, sans-serif");
    }

    #[test]
    fn font_feature_settings_are_parsed_reset_and_inherited() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("font-feature-settings", r#""palt" 1"#);
        assert_eq!(style.font_feature_settings.as_deref(), Some(r#""palt" 1"#));

        let inherited = CssStyle::from_attributes(&[], &style);
        assert_eq!(inherited.font_feature_settings, style.font_feature_settings);

        style.apply_typography_property("font-feature-settings", "normal");
        assert_eq!(style.font_feature_settings, None);
        style.apply_typography_property("font-feature-settings", "  ");
        assert_eq!(style.font_feature_settings, None);
    }

    #[test]
    fn white_space_nowrap_is_parsed_and_inherited() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("white-space", "nowrap");
        assert_eq!(style.white_space, CssWhiteSpace::NoWrap);

        let inherited = CssStyle::from_attributes(&[], &style);
        assert_eq!(inherited.white_space, CssWhiteSpace::NoWrap);

        style.apply_typography_property("white-space", "normal");
        assert_eq!(style.white_space, CssWhiteSpace::Normal);
        style.apply_typography_property("white-space", "unsupported");
        assert_eq!(style.white_space, CssWhiteSpace::Normal);
    }

    #[test]
    fn text_align_handles_center_and_right_and_unknown() {
        let mut style = CssStyle::browser_default();
        style.apply_text_align("end");
        assert_eq!(style.text_align, CssTextAlign::End);
        style.apply_text_align("left");
        assert_eq!(style.text_align, CssTextAlign::Start);

        style.apply_text_align("unsupported");
        assert_eq!(style.text_align, CssTextAlign::Start);
    }

    #[test]
    fn letter_spacing_resolves_normal_and_signed_values() {
        let mut style = CssStyle::browser_default();

        style.apply_letter_spacing("normal");
        assert_eq!(style.letter_spacing, 0.0);

        style.apply_letter_spacing("-0.5px");
        assert_eq!(style.letter_spacing, -0.5);
    }

    #[test]
    fn text_transform_is_parsed_inherited_and_applied_to_unicode_text() {
        let mut style = CssStyle::browser_default();
        style.apply_typography_property("text-transform", "uppercase");
        assert_eq!(style.text_transform, CssTextTransform::Uppercase);
        assert_eq!(style.transformed_text("LibreChat ß"), "LIBRECHAT SS");

        let inherited = CssStyle::from_attributes(&[], &style);
        assert_eq!(inherited.text_transform, CssTextTransform::Uppercase);

        style.apply_typography_property("text-transform", "capitalize");
        assert_eq!(style.transformed_text("hello rust"), "Hello Rust");
        style.apply_typography_property("text-transform", "lowercase");
        assert_eq!(style.transformed_text("KRR"), "krr");
        style.apply_typography_property("text-transform", "unsupported");
        assert_eq!(style.transformed_text("KRR"), "krr");
    }
}
