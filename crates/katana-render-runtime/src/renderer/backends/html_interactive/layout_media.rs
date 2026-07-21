use super::constants::{CONTROL_HEIGHT, MIN_LAYOUT_WIDTH};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::svg::escape_xml;
use super::types::LayoutContext;
use crate::renderer::backends::html_document::{
    EMBEDDED_SVG_HEIGHT_PLACEHOLDER, EMBEDDED_SVG_MARKUP_ATTRIBUTE, EMBEDDED_SVG_WIDTH_PLACEHOLDER,
    EMBEDDED_SVG_X_PLACEHOLDER, EMBEDDED_SVG_Y_PLACEHOLDER,
};

const DEFAULT_IMAGE_MAX_HEIGHT: f32 = 240.0;
const VIEW_BOX_VALUE_COUNT: usize = 4;
const VIEW_BOX_WIDTH_INDEX: usize = 2;
const VIEW_BOX_HEIGHT_INDEX: usize = 3;

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

    pub(super) fn render_embedded_svg(
        &mut self,
        attributes: &[(String, String)],
        layout: LayoutContext<'_>,
    ) -> f32 {
        let start = layout.y + layout.style.margin_top;
        let x = layout.x + layout.style.margin_left;
        let available_width = (layout.width - layout.style.margin_left - layout.style.margin_right)
            .max(MIN_LAYOUT_WIDTH);
        let Some(markup) = attribute(attributes, EMBEDDED_SVG_MARKUP_ATTRIBUTE) else {
            return start + layout.style.margin_bottom;
        };
        let (svg_width, svg_height) = embedded_svg_size(attributes, available_width, layout.style);
        let svg = position_embedded_svg(markup, x, start - self.scroll_y, svg_width, svg_height);
        self.svg.push_str(&svg);
        start + svg_height + layout.style.margin_bottom
    }
}

fn image_source(attributes: &[(String, String)]) -> Option<&str> {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("src"))
        .map(|(_, value)| value.as_str())
}

fn embedded_svg_size(
    attributes: &[(String, String)],
    available_width: f32,
    style: &CssStyle,
) -> (f32, f32) {
    let view_box = attribute(attributes, "viewbox").and_then(parse_view_box);
    let natural_width = attribute(attributes, "width")
        .and_then(|value| parse_svg_length(value, available_width))
        .or_else(|| view_box.map(|(_, _, width, _)| width))
        .unwrap_or(available_width)
        .max(MIN_LAYOUT_WIDTH);
    let natural_height = attribute(attributes, "height")
        .and_then(|value| parse_svg_length(value, available_width))
        .or_else(|| view_box.map(|(_, _, _, height)| height))
        .unwrap_or(DEFAULT_IMAGE_MAX_HEIGHT)
        .max(CONTROL_HEIGHT);
    let styled_width = if style.width.is_some() || style.max_width.is_some() {
        style.box_width(available_width)
    } else {
        natural_width
    };
    let width = styled_width.min(available_width).max(MIN_LAYOUT_WIDTH);
    let height = style
        .height
        .unwrap_or(natural_height * (width / natural_width))
        .max(CONTROL_HEIGHT);
    (width, height)
}

fn parse_svg_length(value: &str, available_width: f32) -> Option<f32> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|percent| percent.is_finite() && *percent > 0.0)
            .map(|percent| available_width * percent / 100.0);
    }
    value
        .strip_suffix("px")
        .unwrap_or(value)
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn parse_view_box(value: &str) -> Option<(f32, f32, f32, f32)> {
    let values = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == VIEW_BOX_VALUE_COUNT
        && values.iter().all(|value| value.is_finite())
        && values[VIEW_BOX_WIDTH_INDEX] > 0.0
        && values[VIEW_BOX_HEIGHT_INDEX] > 0.0)
        .then(|| {
            (
                values[0],
                values[1],
                values[VIEW_BOX_WIDTH_INDEX],
                values[VIEW_BOX_HEIGHT_INDEX],
            )
        })
}

fn position_embedded_svg(markup: &str, x: f32, y: f32, width: f32, height: f32) -> String {
    markup
        .replace(EMBEDDED_SVG_X_PLACEHOLDER, &x.to_string())
        .replace(EMBEDDED_SVG_Y_PLACEHOLDER, &y.to_string())
        .replace(EMBEDDED_SVG_WIDTH_PLACEHOLDER, &width.to_string())
        .replace(EMBEDDED_SVG_HEIGHT_PLACEHOLDER, &height.to_string())
}

fn attribute<'a>(attributes: &'a [(String, String)], expected: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(expected))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
#[path = "layout_media_tests.rs"]
mod tests;
