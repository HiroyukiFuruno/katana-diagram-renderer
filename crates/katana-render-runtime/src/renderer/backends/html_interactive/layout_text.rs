use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::document::{node_text, wrap_text};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{HitTarget, HitTargetKind};

impl HtmlLayoutRenderer {
    pub(super) fn render_container(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        details_node_id: Option<u64>,
    ) -> f32 {
        let start = y + style.margin_top;
        let box_width = style
            .width
            .unwrap_or(width)
            .min(width)
            .max(MIN_LAYOUT_WIDTH);
        let inner_x = x + style.padding;
        let inner_width = (box_width - style.padding * 2.0).max(MIN_LAYOUT_WIDTH);
        let box_start = self.svg.len();
        let bottom = self.render_nodes(
            children,
            inner_x,
            start + style.padding,
            inner_width,
            style,
            details_node_id,
        );
        let height = container_height(bottom, start, style);
        self.insert_box(box_start, x, start, box_width, height, style);
        start + height + style.margin_bottom
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
        let height = self.paint_wrapped_box(&node_text(children), x, start, width, &style);
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
        let height = self.paint_wrapped_box(&node_text(children), x, start, width, &style);
        self.hit_targets.push(HitTarget {
            node_id,
            x,
            y: start,
            width,
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
            (width - style.padding * 2.0).max(MIN_LAYOUT_WIDTH),
            style.font_size,
        );
        let height = text_box_height(&lines, style);
        self.paint_box(x, start, width, height, style);
        self.paint_text_lines(
            &lines,
            x + style.padding,
            start + style.padding + style.font_size,
            style,
        );
        height
    }
}

fn container_height(bottom: f32, start: f32, style: &CssStyle) -> f32 {
    let explicit_height = style.height.unwrap_or(0.0).max(style.min_height);
    (bottom - start + style.padding).max(explicit_height)
}

fn text_box_height(lines: &[String], style: &CssStyle) -> f32 {
    (lines.len() as f32 * style.line_height + style.padding * 2.0).max(style.min_height)
}

fn link_style(style: &CssStyle) -> CssStyle {
    let mut style = style.clone();
    if !style.explicit_color {
        style.color = "#0969da".to_string();
    }
    style.underline = true;
    style
}
