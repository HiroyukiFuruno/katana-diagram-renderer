use super::value::box_sides;
use super::{CssLength, CssStyle};

impl CssStyle {
    pub(super) fn apply_dimensions(&mut self, name: &str, value: &str) {
        match name {
            "width" | "min-width" | "max-width" => self.apply_width_dimension(name, value),
            "height" => self.height = self.block_length(value),
            "min-height" => self.apply_minimum_height(value),
            "max-height" => self.max_height = self.block_length(value),
            _ => {}
        }
    }

    fn apply_width_dimension(&mut self, name: &str, value: &str) {
        let resolved = CssLength::parse(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        );
        match name {
            "width" => self.width = resolved,
            "min-width" => self.min_width = resolved,
            "max-width" => self.max_width = resolved,
            _ => {}
        }
    }

    fn apply_minimum_height(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("auto") {
            self.min_height = 0.0;
            self.automatic_min_height = true;
        } else if let Some(minimum) = self.block_length(value) {
            self.min_height = minimum;
            self.automatic_min_height = false;
        }
    }

    pub(super) fn apply_inset(&mut self, name: &str, value: &str) {
        if name == "inset" {
            let Some([top, right, bottom, left]) = box_sides(
                value,
                self.font_size,
                self.viewport_width,
                self.viewport_height,
                true,
            ) else {
                return;
            };
            self.inset_top = Some(top);
            self.inset_right = Some(right);
            self.inset_bottom = Some(bottom);
            self.inset_left = Some(left);
            return;
        }
        let resolved = self.box_length(value, true);
        match name {
            "top" => self.inset_top = resolved,
            "right" => self.inset_right = resolved,
            "bottom" => self.inset_bottom = resolved,
            "left" => self.inset_left = resolved,
            _ => {}
        }
    }

    fn block_length(&self, value: &str) -> Option<f32> {
        if let Some(percent) = value.trim().strip_suffix('%') {
            let percent = percent.trim().parse::<f32>().ok()?;
            if !percent.is_finite() || percent < 0.0 {
                return None;
            }
            return self
                .percentage_height_basis
                .map(|basis| basis * percent / 100.0);
        }
        self.box_length(value, false)
    }
}

#[cfg(test)]
mod tests {
    use super::CssStyle;

    #[test]
    fn dimensions_ignore_unknown_names_and_restore_auto_minimum_height() {
        let mut style = CssStyle::browser_default();
        style.apply_width_dimension("unknown", "12px");
        style.min_height = 10.0;

        style.apply_minimum_height("auto");

        assert_eq!(style.min_height, 0.0);
        assert!(style.automatic_min_height);
    }

    #[test]
    fn inset_ignores_invalid_shorthand_and_unknown_longhand() {
        let mut style = CssStyle::browser_default();
        style.apply_inset("inset", "1px 2px 3px 4px 5px");
        style.apply_inset("inline-start", "9px");

        assert!(style.inset_top.is_none());
        assert!(style.inset_left.is_none());
    }

    #[test]
    fn percentage_block_length_rejects_negative_and_non_finite_values() {
        let style = CssStyle::browser_default();

        assert!(style.block_length("-1%").is_none());
        assert!(style.block_length("NaN%").is_none());
    }

    #[test]
    fn minimum_height_updates_value_when_length_is_specified() {
        let mut style = CssStyle::browser_default();
        style.automatic_min_height = true;
        style.min_height = 5.0;

        style.apply_minimum_height("10px");

        assert_eq!(style.min_height, 10.0);
        assert!(!style.automatic_min_height);

        style.apply_minimum_height("invalid");
        assert_eq!(style.min_height, 10.0);
        assert!(!style.automatic_min_height);
    }
}
