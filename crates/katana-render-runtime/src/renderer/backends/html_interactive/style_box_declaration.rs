use super::super::document::css_px;
use super::value::{box_sides, css_relative_px};
use super::{CssBoxSizing, CssLength, CssOverflow, CssStyle};

impl CssStyle {
    pub(super) fn apply_box_property(&mut self, name: &str, value: &str) -> bool {
        let name = name.to_ascii_lowercase();
        match name.as_str() {
            "box-sizing" => self.apply_box_sizing(value),
            "overflow" | "overflow-x" | "overflow-y" => self.apply_overflow(value),
            "border-width" => {
                self.border_width = css_px(value).unwrap_or(self.border_width);
            }
            "border-radius" => {
                self.border_radius = self.box_length(value, false).unwrap_or(self.border_radius);
            }
            "padding" | "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
                self.apply_padding_property(&name, value)
            }
            "margin" | "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
                self.apply_margin_property(&name, value);
            }
            "width" | "max-width" | "height" | "min-height" => {
                self.apply_dimensions(&name, value);
            }
            _ => return false,
        }
        true
    }

    fn apply_box_sizing(&mut self, value: &str) {
        self.box_sizing = match value.trim().to_ascii_lowercase().as_str() {
            "border-box" => CssBoxSizing::BorderBox,
            "content-box" => CssBoxSizing::ContentBox,
            _ => self.box_sizing,
        };
    }

    fn apply_overflow(&mut self, value: &str) {
        self.overflow = match value.trim().to_ascii_lowercase().as_str() {
            "hidden" | "clip" | "auto" | "scroll" => CssOverflow::Clip,
            "visible" => CssOverflow::Visible,
            _ => self.overflow,
        };
    }

    fn apply_padding_property(&mut self, name: &str, value: &str) {
        match name {
            "padding" => self.apply_padding(value),
            "padding-top" => {
                self.padding_top = self.box_length(value, false).unwrap_or(self.padding_top);
            }
            "padding-right" => {
                self.padding_right = self.box_length(value, false).unwrap_or(self.padding_right);
            }
            "padding-bottom" => {
                self.padding_bottom = self.box_length(value, false).unwrap_or(self.padding_bottom);
            }
            "padding-left" => {
                self.padding_left = self.box_length(value, false).unwrap_or(self.padding_left);
            }
            _ => {}
        }
    }

    fn apply_margin_property(&mut self, name: &str, value: &str) {
        match name {
            "margin" => self.apply_margin(value),
            "margin-top" => {
                self.margin_top = self.box_length(value, true).unwrap_or(self.margin_top);
            }
            "margin-right" => {
                self.margin_right = self.box_length(value, true).unwrap_or(self.margin_right);
            }
            "margin-bottom" => {
                self.margin_bottom = self.box_length(value, true).unwrap_or(self.margin_bottom);
            }
            "margin-left" => {
                self.margin_left = self.box_length(value, true).unwrap_or(self.margin_left);
            }
            _ => {}
        }
    }

    fn apply_dimensions(&mut self, name: &str, value: &str) {
        match name {
            "width" => self.width = CssLength::parse(value, self.font_size),
            "max-width" => self.max_width = CssLength::parse(value, self.font_size),
            "height" => self.height = self.box_length(value, false),
            "min-height" => {
                self.min_height = self.box_length(value, false).unwrap_or(self.min_height);
            }
            _ => {}
        }
    }

    fn apply_padding(&mut self, value: &str) {
        let Some([top, right, bottom, left]) = box_sides(value, self.font_size, false) else {
            return;
        };
        self.padding_top = top;
        self.padding_right = right;
        self.padding_bottom = bottom;
        self.padding_left = left;
    }

    fn apply_margin(&mut self, value: &str) {
        let Some([top, right, bottom, left]) = box_sides(value, self.font_size, true) else {
            return;
        };
        self.margin_top = top;
        self.margin_right = right;
        self.margin_bottom = bottom;
        self.margin_left = left;
    }

    fn box_length(&self, value: &str, signed: bool) -> Option<f32> {
        css_relative_px(value, self.font_size, signed)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{CssBoxSizing, CssLength, CssOverflow, CssStyle};

    #[test]
    fn apply_box_property_displays_invalid_values_are_ignored() {
        let mut style = CssStyle::browser_default();

        assert!(!style.apply_box_property("unsupported-property", "10px"));
        assert_eq!(style.box_sizing, CssBoxSizing::ContentBox);
        assert_eq!(style.overflow, CssOverflow::Visible);
    }

    #[test]
    fn apply_box_sizing_resolves_known_values_and_keeps_unknown() {
        let mut style = CssStyle::browser_default();

        style.apply_box_property("box-sizing", "border-box");
        assert_eq!(style.box_sizing, CssBoxSizing::BorderBox);

        style.apply_box_property("box-sizing", "unsupported");
        assert_eq!(style.box_sizing, CssBoxSizing::BorderBox);
    }

    #[test]
    fn apply_overflow_sets_clip_for_hidden_like_values_and_keeps_visible() {
        let mut style = CssStyle::browser_default();

        style.apply_box_property("overflow", "hidden");
        assert_eq!(style.overflow, CssOverflow::Clip);

        style.apply_box_property("overflow", "visible");
        assert_eq!(style.overflow, CssOverflow::Visible);

        style.apply_box_property("overflow", "unsupported");
        assert_eq!(style.overflow, CssOverflow::Visible);
    }

    #[test]
    fn apply_padding_property_keeps_each_edge_with_shorthand_and_invalid_value() {
        let mut style = CssStyle::browser_default();
        style.padding_top = 1.0;
        style.padding_right = 2.0;
        style.padding_bottom = 3.0;
        style.padding_left = 4.0;
        style.apply_padding_property("padding", "8px 9px");
        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left
            ],
            [8.0, 9.0, 8.0, 9.0]
        );

        style.apply_padding_property("padding", "1px 2px 3px 4px 5px");
        assert_eq!(
            [
                style.padding_top,
                style.padding_right,
                style.padding_bottom,
                style.padding_left
            ],
            [8.0, 9.0, 8.0, 9.0]
        );
    }

    #[test]
    fn apply_padding_property_drops_unknown_name_unmodified() {
        let mut style = CssStyle::browser_default();
        style.padding_left = 9.0;
        style.apply_padding_property("padding-inline", "4px");
        assert_eq!(style.padding_left, 9.0);
    }

    #[test]
    fn apply_margin_property_drops_unknown_name_unmodified() {
        let mut style = CssStyle::browser_default();
        style.margin_top = 9.0;
        style.apply_margin_property("margin-inline", "4px");
        assert_eq!(style.margin_top, 9.0);
    }

    #[test]
    fn apply_margin_property_sets_each_edge_and_ignores_invalid_shorthand() {
        let mut style = CssStyle::browser_default();
        style.apply_margin_property("margin", "1px 2px 3px 4px");
        assert_eq!(
            [
                style.margin_top,
                style.margin_right,
                style.margin_bottom,
                style.margin_left
            ],
            [1.0, 2.0, 3.0, 4.0]
        );

        style.apply_margin_property("margin", "1px 2px 3px 4px 5px");
        assert_eq!(
            [
                style.margin_top,
                style.margin_right,
                style.margin_bottom,
                style.margin_left
            ],
            [1.0, 2.0, 3.0, 4.0]
        );
    }

    #[test]
    fn apply_dimension_converts_known_longhands_and_preserves_unknown() {
        let mut style = CssStyle::browser_default();
        style.apply_dimensions("width", "120%");
        assert_eq!(style.width, Some(CssLength::Percent(1.2)));
        style.apply_dimensions("max-width", "60px");
        assert_eq!(style.max_width, Some(CssLength::Px(60.0)));
        style.apply_dimensions("height", "7rem");
        assert_eq!(style.height, Some(112.0));
        style.apply_dimensions("min-height", "1em");
        assert_eq!(style.min_height, 16.0);

        style.apply_dimensions("line-height", "42px");
        assert_eq!(style.min_height, 16.0);
    }
}
