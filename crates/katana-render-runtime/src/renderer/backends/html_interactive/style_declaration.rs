use super::super::document::{border_color, css_px};
use super::value::{
    box_sides, css_font_size, css_line_height, css_number, css_relative_px, grid_tracks, is_bold,
};
use super::{CssLength, CssStyle};

impl CssStyle {
    pub(super) fn apply_declaration(&mut self, declaration: &str) {
        let Some((name, value)) = declaration.split_once(':') else {
            return;
        };
        self.apply(name.trim(), value.trim());
    }

    fn apply(&mut self, name: &str, value: &str) {
        match name.to_ascii_lowercase().as_str() {
            "display" => self.display = value.parse().unwrap_or(self.display),
            "color" => self.apply_color(value),
            "background" | "background-color" => self.apply_background(value),
            "border" | "border-color" => self.border = border_color(value),
            _ => self.apply_layout_or_font(name, value),
        }
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
        match name.to_ascii_lowercase().as_str() {
            "font-size" => {
                self.font_size = css_font_size(value, self.font_size).unwrap_or(self.font_size);
            }
            "line-height" => self.apply_line_height(value),
            "font-weight" => self.bold = is_bold(value),
            "text-decoration" => self.underline = value.contains("underline"),
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
            _ => self.apply_box_measurement(name, value),
        }
    }

    pub(super) fn apply_line_height(&mut self, value: &str) {
        let Some((resolved, inherited_factor)) = css_line_height(value, self.font_size) else {
            return;
        };
        self.line_height = resolved;
        self.line_height_factor = inherited_factor;
    }

    fn apply_box_measurement(&mut self, name: &str, value: &str) {
        let name = name.to_ascii_lowercase();
        if name.starts_with("padding") {
            self.apply_padding_property(&name, value);
        } else if name.starts_with("margin") {
            self.apply_margin_property(&name, value);
        } else {
            self.apply_dimensions(&name, value);
        }
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
        match name.to_ascii_lowercase().as_str() {
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
