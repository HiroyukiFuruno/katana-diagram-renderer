use super::constants::{CONTROL_HEIGHT, MIN_LAYOUT_WIDTH};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::svg::escape_xml;

const DEFAULT_IMAGE_MAX_HEIGHT: f32 = 240.0;

impl HtmlLayoutRenderer {
    pub(super) fn render_image(
        &mut self,
        attributes: &[(String, String)],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let start = y + style.margin_top;
        let x = x + style.margin_left;
        let available_width =
            (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let Some(source) = image_source(attributes) else {
            return start + style.margin_bottom;
        };
        let image_width = style
            .box_width(available_width)
            .min(available_width)
            .max(MIN_LAYOUT_WIDTH);
        let image_height = style
            .height
            .unwrap_or(image_width.min(DEFAULT_IMAGE_MAX_HEIGHT))
            .max(CONTROL_HEIGHT);
        let image_y = start - self.scroll_y;
        self.svg.push_str(&format!(
            r#"<image href="{}" x="{x}" y="{image_y}" width="{image_width}" height="{image_height}" preserveAspectRatio="xMidYMid meet"/>"#,
            escape_xml(source)
        ));
        start + image_height + style.margin_bottom
    }
}

fn image_source(attributes: &[(String, String)]) -> Option<&str> {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("src"))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
#[path = "layout_media_tests.rs"]
mod tests;
