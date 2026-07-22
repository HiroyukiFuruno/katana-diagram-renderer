use super::super::constants::{
    H1_FONT_SIZE, H1_MARGIN, H2_FONT_SIZE, H2_MARGIN, H3_FONT_SIZE, H3_MARGIN, PARAGRAPH_MARGIN,
};
use super::CssStyle;

impl CssStyle {
    pub(super) fn apply_tag_metrics(&mut self, tag: &str) {
        match tag {
            "h1" => self.apply_heading(H1_FONT_SIZE, H1_MARGIN),
            "h2" => self.apply_heading(H2_FONT_SIZE, H2_MARGIN),
            "h3" | "h4" | "h5" | "h6" => self.apply_heading(H3_FONT_SIZE, H3_MARGIN),
            "p" => self.margin_bottom += PARAGRAPH_MARGIN,
            "b" | "strong" => self.bold = true,
            "em" | "i" => self.italic = true,
            _ => {}
        }
    }

    fn apply_heading(&mut self, font_size: f32, margin: f32) {
        self.font_size = self.font_size.max(font_size);
        self.bold = true;
        self.margin_top += margin;
        self.margin_bottom += margin;
    }
}
