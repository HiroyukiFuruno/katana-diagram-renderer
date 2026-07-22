use super::value::{css_font_size, css_relative_px, is_bold};
use super::{CssStyle, CssTextAlign};

impl CssStyle {
    pub(super) fn apply_typography_property(&mut self, name: &str, value: &str) -> bool {
        match name.to_ascii_lowercase().as_str() {
            "font-size" => {
                self.font_size = css_font_size(value, self.font_size).unwrap_or(self.font_size);
            }
            "font-weight" => self.bold = is_bold(value),
            "font-family" => self.apply_font_family(value),
            "font-style" => {
                self.italic = matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "italic" | "oblique"
                );
            }
            "text-decoration" => self.underline = value.contains("underline"),
            "text-align" => self.apply_text_align(value),
            "letter-spacing" => self.apply_letter_spacing(value),
            _ => return false,
        }
        true
    }

    fn apply_font_family(&mut self, value: &str) {
        let family = value.trim();
        if !family.is_empty() {
            self.font_family = family.to_string();
        }
    }

    fn apply_text_align(&mut self, value: &str) {
        self.text_align = match value.trim().to_ascii_lowercase().as_str() {
            "center" => CssTextAlign::Center,
            "right" | "end" => CssTextAlign::End,
            "left" | "start" => CssTextAlign::Start,
            _ => self.text_align,
        };
    }

    fn apply_letter_spacing(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("normal") {
            self.letter_spacing = 0.0;
        } else {
            self.letter_spacing =
                css_relative_px(value, self.font_size, true).unwrap_or(self.letter_spacing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, CssTextAlign};

    #[test]
    fn typography_dispatches_known_properties_and_rejects_unknown() {
        let mut style = CssStyle::browser_default();

        assert!(style.apply_typography_property("font-size", "2rem"));
        assert!(style.apply_typography_property("font-weight", "700"));
        assert!(style.bold);
        assert!(style.apply_typography_property("font-style", "italic"));
        assert!(style.italic);

        assert!(!style.apply_typography_property("line-height", "160%"));
    }

    #[test]
    fn font_family_keeps_current_if_empty() {
        let mut style = CssStyle::browser_default();
        style.font_family = "Noto Sans, sans-serif".to_string();

        style.apply_typography_property("font-family", "  ");

        assert_eq!(style.font_family, "Noto Sans, sans-serif");
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
}
