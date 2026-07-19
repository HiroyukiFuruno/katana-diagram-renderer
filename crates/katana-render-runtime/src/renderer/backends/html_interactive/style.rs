use super::constants::{
    BOLD_FONT_WEIGHT_MINIMUM, DEFAULT_FONT_SIZE, DEFAULT_LINE_HEIGHT, H1_FONT_SIZE, H1_MARGIN,
    H2_FONT_SIZE, H2_MARGIN, H3_FONT_SIZE, H3_MARGIN, LINE_HEIGHT_FACTOR, PARAGRAPH_MARGIN,
};
use super::document::{attribute, border_color, css_px};

#[derive(Debug, Clone)]
pub(super) struct CssStyle {
    pub(super) color: String,
    pub(super) background: Option<String>,
    pub(super) border: Option<String>,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    pub(super) padding: f32,
    pub(super) margin_top: f32,
    pub(super) margin_bottom: f32,
    pub(super) min_height: f32,
    pub(super) width: Option<f32>,
    pub(super) height: Option<f32>,
    pub(super) bold: bool,
    pub(super) underline: bool,
    pub(super) display_none: bool,
    pub(super) explicit_color: bool,
    pub(super) explicit_background: bool,
}

impl Default for CssStyle {
    fn default() -> Self {
        Self {
            color: "#1f2328".to_string(),
            background: None,
            border: None,
            font_size: DEFAULT_FONT_SIZE,
            line_height: DEFAULT_LINE_HEIGHT,
            padding: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            min_height: 0.0,
            width: None,
            height: None,
            bold: false,
            underline: false,
            display_none: false,
            explicit_color: false,
            explicit_background: false,
        }
    }
}

impl CssStyle {
    pub(super) fn from_attributes(attributes: &[(String, String)], inherited: &Self) -> Self {
        let mut style = inherited.element_defaults();
        style.display_none = attribute(attributes, "hidden").is_some();
        let Some(source) = attribute(attributes, "style") else {
            return style;
        };
        for declaration in source.split(';') {
            style.apply_declaration(declaration);
        }
        style
    }

    fn element_defaults(&self) -> Self {
        Self {
            color: self.color.clone(),
            font_size: self.font_size,
            line_height: self.line_height,
            ..Self::default()
        }
    }

    fn apply_declaration(&mut self, declaration: &str) {
        let Some((name, value)) = declaration.split_once(':') else {
            return;
        };
        self.apply(name.trim(), value.trim());
    }

    pub(super) fn for_tag(mut self, tag: &str) -> Self {
        self.apply_tag_metrics(tag);
        self.line_height = self.line_height.max(self.font_size * LINE_HEIGHT_FACTOR);
        self
    }

    fn apply_tag_metrics(&mut self, tag: &str) {
        match tag {
            "h1" => self.apply_heading(H1_FONT_SIZE, H1_MARGIN),
            "h2" => self.apply_heading(H2_FONT_SIZE, H2_MARGIN),
            "h3" | "h4" | "h5" | "h6" => self.apply_heading(H3_FONT_SIZE, H3_MARGIN),
            "p" => self.margin_bottom += PARAGRAPH_MARGIN,
            _ => {}
        }
    }

    fn apply_heading(&mut self, font_size: f32, margin: f32) {
        self.font_size = self.font_size.max(font_size);
        self.bold = true;
        self.margin_top += margin;
        self.margin_bottom += margin;
    }

    fn apply(&mut self, name: &str, value: &str) {
        match name.to_ascii_lowercase().as_str() {
            "display" if value.eq_ignore_ascii_case("none") => self.display_none = true,
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
            "font-size" => self.font_size = css_px(value).unwrap_or(self.font_size),
            "line-height" => self.line_height = css_px(value).unwrap_or(self.line_height),
            "font-weight" => self.bold = is_bold(value),
            "text-decoration" => self.underline = value.contains("underline"),
            _ => self.apply_box_measurement(name, value),
        }
    }

    fn apply_box_measurement(&mut self, name: &str, value: &str) {
        match name.to_ascii_lowercase().as_str() {
            "padding" => self.padding = css_px(value).unwrap_or(self.padding),
            "margin" => self.apply_margin(css_px(value).unwrap_or(0.0)),
            "margin-top" => self.margin_top = css_px(value).unwrap_or(self.margin_top),
            "margin-bottom" => self.margin_bottom = css_px(value).unwrap_or(self.margin_bottom),
            "width" => self.width = css_px(value),
            "height" => self.height = css_px(value),
            "min-height" => self.min_height = css_px(value).unwrap_or(self.min_height),
            _ => {}
        }
    }

    fn apply_margin(&mut self, margin: f32) {
        self.margin_top = margin;
        self.margin_bottom = margin;
    }
}

fn is_bold(value: &str) -> bool {
    value.eq_ignore_ascii_case("bold")
        || value
            .parse::<u16>()
            .is_ok_and(|weight| weight >= BOLD_FONT_WEIGHT_MINIMUM)
}
