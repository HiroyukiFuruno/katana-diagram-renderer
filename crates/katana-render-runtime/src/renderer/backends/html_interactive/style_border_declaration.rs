use super::super::document::{border_color, css_px};
use super::CssStyle;

impl CssStyle {
    pub(super) fn apply_border(&mut self, value: &str) {
        self.clear_border_edge_overrides();
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

    pub(super) fn apply_border_color(&mut self, value: &str) {
        let Some(color) = border_color(value) else {
            return;
        };
        self.border = Some(color);
        self.border_top_color = None;
        self.border_right_color = None;
        self.border_bottom_color = None;
        self.border_left_color = None;
    }

    pub(super) fn apply_border_side(&mut self, name: &str, value: &str) {
        let side = name
            .to_ascii_lowercase()
            .strip_prefix("border-")
            .map(str::to_string);
        let Some(side) = side else {
            return;
        };
        let color = border_color(value);
        let current_width = self.border_side_width(&side);
        let width = if value.trim().eq_ignore_ascii_case("none") {
            0.0
        } else {
            value
                .split_whitespace()
                .find_map(css_px)
                .unwrap_or_else(|| if color.is_some() { 1.0 } else { current_width })
        };
        self.set_border_side_width(&side, width);
        if let Some(color) = color {
            self.set_border_side_color(&side, color);
        }
    }

    pub(super) fn apply_border_side_color(&mut self, name: &str, value: &str) {
        let Some(color) = border_color(value) else {
            return;
        };
        let lower_name = name.to_ascii_lowercase();
        let side = lower_name
            .strip_prefix("border-")
            .and_then(|value| value.strip_suffix("-color"));
        if let Some(side) = side {
            self.set_border_side_color(side, color);
        }
    }

    fn clear_border_edge_overrides(&mut self) {
        self.border_top_width = None;
        self.border_right_width = None;
        self.border_bottom_width = None;
        self.border_left_width = None;
        self.border_top_color = None;
        self.border_right_color = None;
        self.border_bottom_color = None;
        self.border_left_color = None;
    }

    fn border_side_width(&self, side: &str) -> f32 {
        match side {
            "top" => self.border_top_width,
            "right" => self.border_right_width,
            "bottom" => self.border_bottom_width,
            "left" => self.border_left_width,
            _ => None,
        }
        .unwrap_or(self.border_width)
    }

    pub(super) fn set_border_side_width(&mut self, side: &str, width: f32) {
        match side {
            "top" => self.border_top_width = Some(width),
            "right" => self.border_right_width = Some(width),
            "bottom" => self.border_bottom_width = Some(width),
            "left" => self.border_left_width = Some(width),
            _ => {}
        }
    }

    fn set_border_side_color(&mut self, side: &str, color: String) {
        match side {
            "top" => self.border_top_color = Some(color),
            "right" => self.border_right_color = Some(color),
            "bottom" => self.border_bottom_color = Some(color),
            "left" => self.border_left_color = Some(color),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn border_color_resets_edge_colors_and_ignores_invalid_values() {
        let mut style = CssStyle::browser_default();
        style.border_top_color = Some("red".to_string());

        style.apply_border_color("");
        assert_eq!(style.border_top_color.as_deref(), Some("red"));

        style.apply_border_color("blue");
        assert_eq!(style.border.as_deref(), Some("blue"));
        assert!(style.border_top_color.is_none());
    }

    #[test]
    fn border_side_handles_none_invalid_names_and_invalid_colors() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side("color", "red");
        style.apply_border_side("border-top", "none");
        assert_eq!(style.border_top_width, Some(0.0));

        style.apply_border_side_color("border-top-color", "");
        assert!(style.border_top_color.is_none());
    }

    #[test]
    fn border_side_without_width_uses_color_default_or_current_width() {
        let mut style = CssStyle::browser_default();
        style.apply_border_side("border-right", "red");
        assert_eq!(style.border_right_width, Some(1.0));

        style.border_width = 4.0;
        style.apply_border_side("border-bottom", "solid");
        assert_eq!(style.border_bottom_width, Some(4.0));
    }

    #[test]
    fn unknown_border_edges_preserve_style() {
        let mut style = CssStyle::browser_default();
        style.border_width = 3.0;

        assert_eq!(style.border_side_width("diagonal"), 3.0);
        style.set_border_side_width("diagonal", 7.0);
        style.set_border_side_color("diagonal", "red".to_string());
        assert_eq!(style.border_width, 3.0);
    }
}
