use super::super::constants::{
    FONT_WEIGHT_BOLD, H1_FONT_SIZE, H1_MARGIN, H2_FONT_SIZE, H2_MARGIN, H3_FONT_SIZE, H3_MARGIN,
    PARAGRAPH_MARGIN,
};
use super::CssStyle;

impl CssStyle {
    pub(super) fn apply_tag_metrics(&mut self, tag: &str) {
        if is_phrasing_tag(tag) {
            self.inline_block = true;
            self.inline_atomic = false;
        }
        match tag {
            "h1" => self.apply_heading(H1_FONT_SIZE, H1_MARGIN),
            "h2" => self.apply_heading(H2_FONT_SIZE, H2_MARGIN),
            "h3" | "h4" | "h5" | "h6" => self.apply_heading(H3_FONT_SIZE, H3_MARGIN),
            "p" => self.margin_bottom += PARAGRAPH_MARGIN,
            "b" | "strong" => self.font_weight = FONT_WEIGHT_BOLD,
            "em" | "i" => self.italic = true,
            _ => {}
        }
    }

    fn apply_heading(&mut self, font_size: f32, margin: f32) {
        self.font_size = self.font_size.max(font_size);
        self.font_weight = FONT_WEIGHT_BOLD;
        self.margin_top += margin;
        self.margin_bottom += margin;
    }
}

fn is_phrasing_tag(tag: &str) -> bool {
    matches!(
        tag,
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "cite"
            | "code"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "mark"
            | "q"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
    )
}
