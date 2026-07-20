/* WHY: SVG rasterization utility.
Uses `resvg` + `usvg` to convert SVG text to an RGBA pixel buffer.
Returns the result as raw bytes compatible with egui's `ColorImage`. */

use resvg::{render, usvg};
use tiny_skia::Pixmap;

#[path = "svg_rasterize_font.rs"]
mod font;

use font::{html_rasterizer_options, rasterizer_options};

const MAX_RASTERIZED_SVG_EDGE: f32 = 8192.0;
const LIGHT_DARK_FUNCTION: &str = "light-dark(";

#[derive(Debug, Clone)]
pub struct RasterizedSvg {
    pub width: u32,
    pub height: u32,
    pub display_width: f32,
    pub display_height: f32,
    pub rgba: Vec<u8>,
}

pub struct SvgRasterizeOps;

impl SvgRasterizeOps {
    pub fn preprocess_for_rasterizer(svg_text: &str) -> String {
        let with_xml_entities = normalize_html_entities_for_xml(svg_text);
        let without_foreign_objects = strip_foreign_objects(&with_xml_entities);
        resolve_light_dark_functions(&without_foreign_objects)
    }

    pub fn rasterize_svg(svg_text: &str, scale: f32) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_with_options(svg_text, scale, &rasterizer_options())
    }

    pub(crate) fn rasterize_html_svg(
        svg_text: &str,
        scale: f32,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        Self::rasterize_with_options(svg_text, scale, &html_rasterizer_options())
    }

    fn rasterize_with_options(
        svg_text: &str,
        scale: f32,
        options: &usvg::Options<'_>,
    ) -> Result<RasterizedSvg, SvgRasterizeError> {
        let compatible_svg = Self::preprocess_for_rasterizer(svg_text);
        let tree = usvg::Tree::from_str(&compatible_svg, options)
            .map_err(|e| SvgRasterizeError::ParseFailed(e.to_string()))?;
        let raster = RasterTarget::new(tree.size(), scale);
        let pixmap = raster.render(&tree)?;
        Ok(RasterizedSvg {
            width: raster.width,
            height: raster.height,
            display_width: raster.display_width,
            display_height: raster.display_height,
            rgba: pixmap.take(),
        })
    }
}

struct RasterTarget {
    display_width: f32,
    display_height: f32,
    effective_scale: f32,
    width: u32,
    height: u32,
}

impl RasterTarget {
    fn new(size: usvg::Size, scale: f32) -> Self {
        let display_width = size.width();
        let display_height = size.height();
        let effective_scale = effective_scale(display_width, display_height, scale);
        Self {
            display_width,
            display_height,
            effective_scale,
            width: ((display_width * effective_scale).ceil() as u32).max(1),
            height: ((display_height * effective_scale).ceil() as u32).max(1),
        }
    }

    fn render(&self, tree: &usvg::Tree) -> Result<Pixmap, SvgRasterizeError> {
        let Some(mut pixmap) = Pixmap::new(self.width, self.height) else {
            return Err(SvgRasterizeError::RasterizeFailed(
                "failed to allocate SVG pixmap".to_string(),
            ));
        };
        let transform =
            tiny_skia::Transform::from_scale(self.effective_scale, self.effective_scale);
        render(tree, transform, &mut pixmap.as_mut());
        Ok(pixmap)
    }
}

fn normalize_html_entities_for_xml(svg_text: &str) -> String {
    svg_text.replace("&nbsp;", "&#160;")
}

fn strip_foreign_objects(svg_text: &str) -> String {
    let mut output = String::with_capacity(svg_text.len());
    let mut remaining = svg_text;
    while let Some(start) = remaining.to_ascii_lowercase().find("<foreignobject") {
        output.push_str(&remaining[..start]);
        let after_open = &remaining[start..];
        let lower_after_open = after_open.to_ascii_lowercase();
        if let Some(self_close) = lower_after_open.find("/>") {
            remaining = &after_open[self_close + "/>".len()..];
            continue;
        }
        let Some(close) = lower_after_open.find("</foreignobject>") else {
            output.push_str(after_open);
            return output;
        };
        remaining = &after_open[close + "</foreignobject>".len()..];
    }
    output.push_str(remaining);
    output
}

fn resolve_light_dark_functions(svg_text: &str) -> String {
    let mut result = String::with_capacity(svg_text.len());
    let mut remaining = svg_text;
    while let Some(start) = find_light_dark_function(remaining) {
        let content_start = start + LIGHT_DARK_FUNCTION.len();
        result.push_str(&remaining[..start]);
        let Some((content_end, light_color)) =
            parse_light_dark_function(&remaining[content_start..])
        else {
            result.push_str(&remaining[start..content_start]);
            remaining = &remaining[content_start..];
            continue;
        };
        result.push_str(light_color.trim());
        remaining = &remaining[content_start + content_end + 1..];
    }
    result.push_str(remaining);
    result
}

fn find_light_dark_function(text: &str) -> Option<usize> {
    text.to_ascii_lowercase().find(LIGHT_DARK_FUNCTION)
}

fn parse_light_dark_function(content: &str) -> Option<(usize, &str)> {
    let mut depth = 0usize;
    let mut comma = None;
    for (index, character) in content.char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return comma.map(|comma_index| (index, &content[..comma_index])),
            ')' => depth -= 1,
            ',' if depth == 0 && comma.is_none() => comma = Some(index),
            _ => {}
        }
    }
    None
}

fn effective_scale(width: f32, height: f32, requested_scale: f32) -> f32 {
    let positive_scale = requested_scale.max(f32::MIN_POSITIVE);
    let width_scale = MAX_RASTERIZED_SVG_EDGE / width.max(1.0);
    let height_scale = MAX_RASTERIZED_SVG_EDGE / height.max(1.0);
    positive_scale.min(width_scale).min(height_scale)
}

#[derive(Debug, thiserror::Error)]
pub enum SvgRasterizeError {
    #[error("Failed to parse SVG: {0}")]
    ParseFailed(String),
    #[error("Failed to rasterize SVG: {0}")]
    RasterizeFailed(String),
}

#[cfg(test)]
#[path = "svg_rasterize_tests.rs"]
mod tests;
