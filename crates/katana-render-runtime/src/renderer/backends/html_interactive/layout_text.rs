use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::document::{node_text, wrap_text_with_style};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::text_metrics::text_width;
use super::types::{HitTarget, HitTargetKind};

impl HtmlLayoutRenderer {
    pub(super) fn render_label(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let start = y + style.margin_top;
        let box_x = x + style.margin_left;
        let available = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let box_width = text_box_width(&node_text(children), available, style);
        let height = self.paint_wrapped_box(&node_text(children), box_x, start, box_width, style);
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
        let available = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let box_width = text_box_width(&node_text(children), available, &style);
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
        if text.trim().is_empty() {
            return y;
        }
        let lines = wrap_text_with_style(text, width, style);
        self.paint_text_lines(&lines, x, width, y + style.font_size, style);
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
        let lines = wrap_text_with_style(
            text,
            style.content_width(width).max(MIN_LAYOUT_WIDTH),
            style,
        );
        let height = text_box_height(&lines, style);
        self.paint_box(x, start, width, height, style);
        let content_x = x + style.border_left_width() + style.padding_left;
        let content_width = style.content_width(width).max(MIN_LAYOUT_WIDTH);
        self.paint_text_lines(
            &lines,
            content_x,
            content_width,
            start + style.border_top_width() + style.padding_top + style.font_size,
            style,
        );
        height
    }
}

fn text_box_width(text: &str, available: f32, style: &CssStyle) -> f32 {
    let explicit = style.box_width(available).min(available);
    if !style.inline_block || style.width.is_some() || style.max_width.is_some() {
        return explicit.max(MIN_LAYOUT_WIDTH);
    }
    let content = text_width(text, style);
    style
        .outer_width(content)
        .min(available)
        .max(MIN_LAYOUT_WIDTH)
}

fn text_box_height(lines: &[String], style: &CssStyle) -> f32 {
    let content_height = lines.len() as f32 * style.line_height;
    style.height.map_or_else(
        || {
            style
                .outer_height(content_height)
                .max(style.minimum_outer_height())
        },
        |height| style.outer_height(height).max(style.minimum_outer_height()),
    )
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
    use super::super::super::html_browser::HtmlBrowserViewport;
    use super::{CssStyle, HtmlLayoutRenderer, text_box_width};
    use std::collections::HashMap;

    #[test]
    fn inline_block_text_shrinks_to_content_and_respects_explicit_width() {
        let mut style = CssStyle::browser_default();
        style.inline_block = true;
        style.padding_left = 6.0;
        style.padding_right = 6.0;
        assert!(text_box_width("Open link", 300.0, &style) < 100.0);

        style.width = Some(super::super::style::CssLength::Px(140.0));
        assert_eq!(text_box_width("Open link", 300.0, &style), 152.0);
    }

    #[test]
    fn empty_text_preserves_vertical_position() {
        let viewport = HtmlBrowserViewport {
            width: 100,
            height: 100,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);

        assert_eq!(
            renderer.render_text(" \n ", 0.0, 42.0, 100.0, &CssStyle::browser_default()),
            42.0
        );
    }
}
