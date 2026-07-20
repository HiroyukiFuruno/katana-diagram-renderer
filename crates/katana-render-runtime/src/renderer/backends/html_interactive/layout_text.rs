use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::document::{node_text, wrap_text};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{DetailsContext, HitTarget, HitTargetKind};

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
        let geometry = container_geometry(x, y, width, style);
        let box_start = self.svg.len();
        let bottom = self.render_container_children(children, &geometry, style, details);
        let height = container_height(bottom, geometry.start, style);
        self.insert_box(
            box_start,
            geometry.box_x,
            geometry.start,
            geometry.box_width,
            height,
            style,
        );
        geometry.start + height + style.margin_bottom
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
            geometry.start + style.padding_top,
            geometry.inner_width,
            style,
            details,
        );
        accept_flow_result(&mut self.layout_error, result, geometry.start)
    }

    pub(super) fn render_label(
        &mut self,
        tag: &str,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let style = style.clone().for_tag(tag);
        let start = y + style.margin_top;
        let box_x = x + style.margin_left;
        let box_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let height = self.paint_wrapped_box(&node_text(children), box_x, start, box_width, &style);
        start + height + style.margin_bottom
    }

    pub(super) fn render_link(
        &mut self,
        node_id: u64,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let style = link_style(style);
        let start = y + style.margin_top;
        let box_x = x + style.margin_left;
        let box_width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let height = self.paint_wrapped_box(&node_text(children), box_x, start, box_width, &style);
        self.hit_targets.push(HitTarget {
            node_id,
            x: box_x,
            y: start,
            width: box_width,
            height,
            kind: HitTargetKind::Click,
        });
        start + height + style.margin_bottom
    }

    pub(super) fn render_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let lines = wrap_text(text, width, style.font_size);
        self.paint_text_lines(&lines, x, y + style.font_size, style);
        y + lines.len() as f32 * style.line_height
    }

    fn paint_wrapped_box(
        &mut self,
        text: &str,
        x: f32,
        start: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let lines = wrap_text(
            text,
            (width - style.padding_left - style.padding_right).max(MIN_LAYOUT_WIDTH),
            style.font_size,
        );
        let height = text_box_height(&lines, style);
        self.paint_box(x, start, width, height, style);
        self.paint_text_lines(
            &lines,
            x + style.padding_left,
            start + style.padding_top + style.font_size,
            style,
        );
        height
    }
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
        inner_x: box_x + style.padding_left,
        inner_width: (box_width - style.padding_left - style.padding_right).max(MIN_LAYOUT_WIDTH),
    }
}

fn container_height(bottom: f32, start: f32, style: &CssStyle) -> f32 {
    let explicit_height = style.height.unwrap_or(0.0).max(style.min_height);
    (bottom - start + style.padding_bottom).max(explicit_height)
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

fn text_box_height(lines: &[String], style: &CssStyle) -> f32 {
    (lines.len() as f32 * style.line_height + style.padding_top + style.padding_bottom)
        .max(style.min_height)
}

fn link_style(style: &CssStyle) -> CssStyle {
    let mut style = style.clone();
    if !style.explicit_color {
        style.color = "#0969da".to_string();
    }
    style.underline = true;
    style
}

#[cfg(test)]
mod tests {
    use super::accept_flow_result;

    #[test]
    fn flow_errors_are_recorded_at_the_container_start() {
        let mut layout_error = None;

        assert_eq!(
            accept_flow_result(&mut layout_error, Err("taffy failed".to_string()), 12.0),
            12.0
        );
        assert_eq!(layout_error, Some("taffy failed".to_string()));
    }
}
