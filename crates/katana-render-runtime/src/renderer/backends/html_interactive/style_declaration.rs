use super::super::document::{border_color, css_px};
use super::CssStyle;
use super::value::{css_line_height, css_number, grid_tracks};

impl CssStyle {
    pub(super) fn apply(&mut self, name: &str, value: &str) {
        match name.to_ascii_lowercase().as_str() {
            "display" => self.apply_display(value),
            "color" => self.apply_color(value),
            "background" | "background-color" => self.apply_background(value),
            "border" => self.apply_border(value),
            "border-color" => self.border = border_color(value),
            _ => self.apply_layout_or_font(name, value),
        }
    }

    fn apply_display(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("inline-block") {
            self.display = taffy::style::Display::Block;
            self.inline_block = true;
            return;
        }
        let Ok(display) = value.parse() else {
            return;
        };
        self.display = display;
        self.inline_block = false;
    }

    fn apply_color(&mut self, value: &str) {
        self.color = value.to_string();
        self.explicit_color = true;
    }

    fn apply_background(&mut self, value: &str) {
        self.background = Some(value.to_string());
        self.explicit_background = true;
    }

    fn apply_border(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("none") {
            self.border = None;
            self.border_width = 0.0;
            return;
        }
        self.border = border_color(value).or_else(|| self.border.clone());
        self.border_width = value
            .split_whitespace()
            .find_map(css_px)
            .unwrap_or_else(|| {
                if self.border.is_some() {
                    1.0
                } else {
                    self.border_width
                }
            });
    }

    fn apply_layout_or_font(&mut self, name: &str, value: &str) {
        if self.apply_box_property(name, value) || self.apply_typography_property(name, value) {
            return;
        }
        match name.to_ascii_lowercase().as_str() {
            "line-height" => self.apply_line_height(value),
            "gap" => self.gap = css_px(value).unwrap_or(self.gap),
            "flex-direction" => {
                self.flex_direction = value.parse().unwrap_or(self.flex_direction);
            }
            "flex-wrap" => self.flex_wrap = value.parse().unwrap_or(self.flex_wrap),
            "flex-grow" => self.flex_grow = css_number(value).unwrap_or(self.flex_grow),
            "flex-shrink" => self.flex_shrink = css_number(value).unwrap_or(self.flex_shrink),
            "align-items" => self.align_items = value.parse().ok().or(self.align_items),
            "justify-content" => {
                self.justify_content = value.parse().ok().or(self.justify_content);
            }
            "grid-template-columns" => {
                self.grid_template_columns = grid_tracks(value, self.font_size)
                    .unwrap_or_else(|| self.grid_template_columns.clone());
            }
            _ => {}
        }
    }

    pub(super) fn apply_line_height(&mut self, value: &str) {
        let Some((resolved, inherited_factor)) = css_line_height(value, self.font_size) else {
            return;
        };
        self.line_height = resolved;
        self.line_height_factor = inherited_factor;
    }
}

#[cfg(test)]
mod tests {
    use super::{super::CssOverflow, CssStyle};

    #[test]
    fn border_none_clears_border_state() {
        let mut style = CssStyle::browser_default();
        style.border = Some("blue".to_string());
        style.border_width = 4.0;

        style.apply_border("none");

        assert!(style.border.is_none());
        assert_eq!(style.border_width, 0.0);
    }

    #[test]
    fn border_without_explicit_width_defaults_to_one_when_color_is_set() {
        let mut style = CssStyle::browser_default();
        style.apply_border("#123");

        assert_eq!(style.border, Some("#123".to_string()));
        assert_eq!(style.border_width, 1.0);
    }

    #[test]
    fn border_with_length_parses_width_component() {
        let mut style = CssStyle::browser_default();
        style.apply_border("2px solid #123");

        assert_eq!(style.border, Some("#123".to_string()));
        assert_eq!(style.border_width, 2.0);
    }

    #[test]
    fn invalid_border_preserves_width_when_no_color_or_length_is_present() {
        let mut style = CssStyle::browser_default();
        style.border_width = 7.0;

        style.apply_border("solid");

        assert!(style.border.is_none());
        assert_eq!(style.border_width, 7.0);
    }

    #[test]
    fn apply_dispatches_to_box_or_typography_rules() {
        let mut style = CssStyle::browser_default();
        style.apply("overflow", "hidden");
        assert_eq!(style.overflow, CssOverflow::Clip);
        style.apply("font-family", "Inter");
        assert_eq!(style.font_family, "Inter");
        style.apply("unknown-property", "42");
    }

    #[test]
    fn inline_block_preserves_shrink_to_fit_display_semantics() {
        let mut style = CssStyle::browser_default();

        style.apply("display", "inline-block");
        assert_eq!(style.display, taffy::style::Display::Block);
        assert!(style.inline_block);

        style.apply("display", "grid");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(!style.inline_block);

        style.apply("display", "unsupported");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(!style.inline_block);
    }
}
