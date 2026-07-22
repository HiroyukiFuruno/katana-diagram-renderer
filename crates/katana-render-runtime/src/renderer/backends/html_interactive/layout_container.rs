use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::document::node_text;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::DetailsContext;

struct ContainerGeometry {
    start: f32,
    box_x: f32,
    box_width: f32,
    inner_x: f32,
    inner_width: f32,
}

impl HtmlLayoutRenderer {
    pub(super) fn render_container(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        let width = inline_container_width(children, width, style);
        let geometry = container_geometry(x, y, width, style);
        let box_start = self.svg.len();
        let content_start = self.svg.len();
        let bottom = self.render_container_children(children, &geometry, style, details);
        let height = container_height(bottom, geometry.start, style);
        self.paint_container_box(box_start, content_start, &geometry, height, style);
        geometry.start + height + style.margin_bottom
    }

    fn paint_container_box(
        &mut self,
        box_start: usize,
        content_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        if style.clips_overflow() {
            self.clip_painted_range(
                content_start,
                geometry.box_x,
                geometry.start,
                geometry.box_width,
                height,
                style.border_radius,
            );
        }
        self.insert_box(
            box_start,
            geometry.box_x,
            geometry.start,
            geometry.box_width,
            height,
            style,
        );
    }

    fn render_container_children(
        &mut self,
        children: &[HtmlDocumentNode],
        geometry: &ContainerGeometry,
        style: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        let result = self.render_flow_children(
            children,
            geometry.inner_x,
            geometry.start + style.border_width + style.padding_top,
            geometry.inner_width,
            style,
            details,
        );
        accept_flow_result(&mut self.layout_error, result, geometry.start)
    }
}

fn inline_container_width(children: &[HtmlDocumentNode], available: f32, style: &CssStyle) -> f32 {
    if !style.inline_block || style.width.is_some() || style.max_width.is_some() {
        return available;
    }
    let text = node_text(children);
    let content = text.chars().count() as f32
        * style.font_size
        * super::constants::TEXT_CHARACTER_WIDTH_FACTOR;
    style
        .outer_width(content)
        .min(available)
        .max(MIN_LAYOUT_WIDTH)
}

fn container_geometry(x: f32, y: f32, width: f32, style: &CssStyle) -> ContainerGeometry {
    let start = y + style.margin_top;
    let box_x = x + style.margin_left;
    let available_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let box_width = style
        .box_width(available_width)
        .min(available_width)
        .max(MIN_LAYOUT_WIDTH);
    ContainerGeometry {
        start,
        box_x,
        box_width,
        inner_x: box_x + style.border_width + style.padding_left,
        inner_width: style.content_width(box_width).max(MIN_LAYOUT_WIDTH),
    }
}

fn container_height(bottom: f32, start: f32, style: &CssStyle) -> f32 {
    let natural = bottom - start + style.padding_bottom + style.border_width;
    style.height.map_or_else(
        || natural.max(style.minimum_outer_height()),
        |height| style.outer_height(height).max(style.minimum_outer_height()),
    )
}

fn accept_flow_result(
    layout_error: &mut Option<String>,
    result: Result<f32, String>,
    start: f32,
) -> f32 {
    match result {
        Ok(bottom) => bottom,
        Err(error) => {
            *layout_error = Some(error);
            start
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, HtmlDocumentNode, accept_flow_result, inline_container_width};

    #[test]
    fn flow_errors_are_recorded_at_the_container_start() {
        let mut layout_error = None;

        assert_eq!(
            accept_flow_result(&mut layout_error, Err("taffy failed".to_string()), 12.0),
            12.0
        );
        assert_eq!(layout_error, Some("taffy failed".to_string()));
    }

    #[test]
    fn inline_container_shrinks_to_text_content() {
        let children = [HtmlDocumentNode::Text("Compact label".to_string())];
        let mut style = CssStyle::browser_default();
        style.inline_block = true;

        assert!(inline_container_width(&children, 300.0, &style) < 120.0);
        assert_eq!(inline_container_width(&children, 40.0, &style), 40.0);
    }
}
