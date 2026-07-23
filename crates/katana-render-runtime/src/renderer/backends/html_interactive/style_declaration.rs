use super::super::document::css_px;
use super::value::{css_line_height, css_number, grid_tracks};
use super::{CssPosition, CssStyle};

impl CssStyle {
    pub(super) fn apply(&mut self, name: &str, value: &str) {
        match name.to_ascii_lowercase().as_str() {
            "display" => self.apply_display(value),
            "color" => self.apply_color(value),
            "background" | "background-color" => self.apply_background(value),
            "box-shadow" => self.apply_box_shadow(value),
            "opacity" => self.apply_opacity(value),
            "border" => self.apply_border(value),
            "border-color" => self.apply_border_color(value),
            "border-top" | "border-right" | "border-bottom" | "border-left" => {
                self.apply_border_side(name, value)
            }
            "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color" => self.apply_border_side_color(name, value),
            "position" => self.apply_position(value),
            "z-index" => self.apply_z_index(value),
            "list-style" | "list-style-type" => self.apply_list_style(value),
            _ => self.apply_layout_or_font(name, value),
        }
    }

    fn apply_list_style(&mut self, value: &str) {
        let mut recognized = false;
        for token in value.split_whitespace() {
            match token.to_ascii_lowercase().as_str() {
                "none" => {
                    self.list_style_none = true;
                    recognized = true;
                }
                "disc" | "circle" | "square" | "decimal" => {
                    self.list_style_none = false;
                    recognized = true;
                }
                _ => {}
            }
        }
        if !recognized && value.trim().eq_ignore_ascii_case("initial") {
            self.list_style_none = false;
        }
    }

    fn apply_opacity(&mut self, value: &str) {
        if let Ok(opacity) = value.trim().parse::<f32>()
            && opacity.is_finite()
        {
            self.opacity = opacity.clamp(0.0, 1.0);
        }
    }

    fn apply_z_index(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("auto") {
            self.z_index = None;
            return;
        }
        let Ok(z_index) = value.trim().parse::<i32>() else {
            return;
        };
        self.z_index = Some(z_index);
    }

    fn apply_position(&mut self, value: &str) {
        self.position = match value.trim().to_ascii_lowercase().as_str() {
            "static" => CssPosition::Static,
            "relative" => CssPosition::Relative,
            "absolute" => CssPosition::Absolute,
            "fixed" => CssPosition::Fixed,
            _ => self.position,
        };
    }

    fn apply_display(&mut self, value: &str) {
        match value.trim().to_ascii_lowercase().as_str() {
            "inline" | "inline-block" => {
                self.display = taffy::style::Display::Block;
                self.inline_block = true;
                return;
            }
            "inline-flex" => {
                self.display = taffy::style::Display::Flex;
                self.inline_block = true;
                return;
            }
            "inline-grid" => {
                self.display = taffy::style::Display::Grid;
                self.inline_block = true;
                return;
            }
            _ => {}
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
            "flex-basis" => self.apply_flex_basis(value),
            "flex" => self.apply_flex(value),
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
        let Some((resolved, inherited_factor)) = css_line_height(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        ) else {
            return;
        };
        self.line_height = resolved;
        self.line_height_factor = inherited_factor;
    }
}

#[cfg(test)]
mod tests {
    use super::{super::CssOverflow, CssPosition, CssStyle};

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
    fn outer_box_shadow_keeps_offsets_blur_spread_and_function_color() {
        let mut style = CssStyle::browser_default();
        style.apply("box-shadow", "2px 10px 28px 3px rgba(15, 40, 89, 0.14)");

        assert_eq!(
            style.box_shadow.as_ref().map(|shadow| (
                shadow.offset_x,
                shadow.offset_y,
                shadow.blur_radius,
                shadow.spread_radius,
                shadow.color.as_str(),
            )),
            Some((2.0, 10.0, 28.0, 3.0, "rgba(15, 40, 89, 0.14)"))
        );

        style.apply("box-shadow", "inset 0 0 2px red");
        assert!(style.box_shadow.is_some());
        style.apply("box-shadow", "none");
        assert!(style.box_shadow.is_none());
    }

    #[test]
    fn list_style_none_is_inherited_and_can_be_restored() {
        let mut parent = CssStyle::browser_default();
        parent.apply("list-style", "none");
        assert!(parent.list_style_none);

        let child = CssStyle::from_attributes(&[], &parent);
        assert!(child.list_style_none);

        parent.apply("list-style-type", "disc");
        assert!(!parent.list_style_none);
        parent.apply("list-style", "invalid");
        assert!(!parent.list_style_none);
        parent.apply("list-style", "initial");
        assert!(!parent.list_style_none);
    }

    #[test]
    fn positioning_declarations_handle_auto_invalid_and_inline_grid() {
        let mut style = CssStyle::browser_default();
        style.apply("z-index", "4");
        style.apply("z-index", "auto");
        assert!(style.z_index.is_none());

        style.apply("z-index", "invalid");
        style.apply("position", "invalid");
        assert_eq!(style.position, CssPosition::Static);

        style.apply("display", "inline-grid");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(style.inline_block);
    }

    #[test]
    fn inline_block_preserves_shrink_to_fit_display_semantics() {
        let mut style = CssStyle::browser_default();

        style.apply("display", "inline-block");
        assert_eq!(style.display, taffy::style::Display::Block);
        assert!(style.inline_block);

        style.apply("display", "inline");
        assert_eq!(style.display, taffy::style::Display::Block);
        assert!(style.inline_block);

        style.apply("display", "inline-flex");
        assert_eq!(style.display, taffy::style::Display::Flex);
        assert!(style.inline_block);
    }

    #[test]
    fn block_display_clears_inline_shrink_to_fit_semantics() {
        let mut style = CssStyle::browser_default();
        style.apply("display", "inline-flex");

        style.apply("display", "grid");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(!style.inline_block);

        style.apply("display", "unsupported");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(!style.inline_block);
    }
}
