use super::super::html_document::HtmlDocumentNode;
use super::constants::{LAYOUT_FLOAT_EPSILON, MIN_LAYOUT_WIDTH};
use super::document::wrap_text_with_initial_width;
use super::layout::HtmlLayoutRenderer;
use super::style::{CssStyle, CssTextAlign};
use super::types::DetailsContext;

#[path = "layout_inline_measure.rs"]
mod measure;

use measure::{inline_node_width, inline_run_start_x, inline_text_width, visible_inline_line};

struct InlineFlowState {
    x: f32,
    width: f32,
    cursor_x: f32,
    y: f32,
    bottom: f32,
    has_items: bool,
}

impl InlineFlowState {
    fn new(x: f32, y: f32, width: f32) -> Self {
        Self {
            x,
            width,
            cursor_x: x,
            y,
            bottom: y,
            has_items: false,
        }
    }

    fn bottom(&self) -> f32 {
        if self.has_items { self.bottom } else { self.y }
    }
}

impl HtmlLayoutRenderer {
    pub(super) fn render_nodes(
        &mut self,
        nodes: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        let mut inline = InlineFlowState::new(x, y, width);
        for index in 0..nodes.len() {
            self.render_inline_or_block_node(nodes, index, inherited, details, &mut inline);
        }
        inline.bottom()
    }

    fn render_inline_or_block_node(
        &mut self,
        nodes: &[HtmlDocumentNode],
        index: usize,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        let node = &nodes[index];
        if matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty()) {
            return;
        }
        if !inline.has_items {
            inline.cursor_x =
                inline_run_start_x(&nodes[index..], inline.x, inline.width, inherited);
        }
        if let HtmlDocumentNode::Text(text) = node {
            self.render_inline_text(text, inherited, inline);
        } else if let Some(width) = inline_node_width(node, inherited, inline.width) {
            self.render_inline_node(node, width, inherited, details, inline);
        } else {
            self.render_block_node(node, inherited, details, inline);
        }
    }

    fn render_inline_node(
        &mut self,
        node: &HtmlDocumentNode,
        inline_width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        if inline.has_items
            && inline.cursor_x + inline_width > inline.x + inline.width + LAYOUT_FLOAT_EPSILON
        {
            inline.y = inline.bottom;
            inline.cursor_x = inline.x;
            inline.bottom = inline.y;
        }
        inline.bottom = inline.bottom.max(self.render_node(
            node,
            inline.cursor_x,
            inline.y,
            inline_width,
            inherited,
            details,
        ));
        inline.cursor_x += inline_width;
        inline.has_items = true;
    }

    fn render_inline_text(&mut self, text: &str, style: &CssStyle, inline: &mut InlineFlowState) {
        let initial_x = inline.cursor_x;
        let initial_y = inline.y;
        let remaining_width = (inline.x + inline.width - initial_x).max(MIN_LAYOUT_WIDTH);
        let lines = wrap_text_with_initial_width(text, remaining_width, inline.width, style);
        self.paint_inline_lines(&lines, initial_x, initial_y, style, inline);
        advance_inline_text(inline, text, &lines, remaining_width, initial_y, style);
        inline.has_items = true;
    }

    fn paint_inline_lines(
        &mut self,
        lines: &[String],
        initial_x: f32,
        initial_y: f32,
        style: &CssStyle,
        inline: &InlineFlowState,
    ) {
        let mut paint_style = style.clone();
        paint_style.text_align = CssTextAlign::Start;
        for (index, line) in lines.iter().enumerate() {
            let line_x = if index == 0 { initial_x } else { inline.x };
            let (leading_width, visible_line) = visible_inline_line(line, style);
            let line_x = line_x + leading_width;
            let line_y = initial_y + index as f32 * style.line_height;
            let visible_line = visible_line.to_string();
            self.paint_text_lines(
                std::slice::from_ref(&visible_line),
                line_x,
                (inline.x + inline.width - line_x).max(MIN_LAYOUT_WIDTH),
                line_y + style.font_size,
                &paint_style,
            );
        }
    }
}

fn advance_inline_text(
    inline: &mut InlineFlowState,
    text: &str,
    lines: &[String],
    remaining_width: f32,
    initial_y: f32,
    style: &CssStyle,
) {
    inline.bottom = inline
        .bottom
        .max(initial_y + lines.len() as f32 * style.line_height);
    if lines.len() == 1 {
        inline.cursor_x += inline_text_width(text, style).min(remaining_width);
    } else {
        inline.y = initial_y + (lines.len() - 1) as f32 * style.line_height;
        inline.cursor_x = inline.x
            + lines
                .last()
                .map(|line| inline_text_width(line, style))
                .unwrap_or(0.0);
    }
}

impl HtmlLayoutRenderer {
    fn render_block_node(
        &mut self,
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        if inline.has_items {
            inline.y = inline.bottom;
            inline.cursor_x = inline.x;
            inline.has_items = false;
        }
        inline.y = self.render_node(node, inline.x, inline.y, inline.width, inherited, details);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::html_browser::HtmlBrowserViewport;
    use super::super::types::DetailsContext;
    use super::{
        CssStyle, HtmlDocumentNode, HtmlLayoutRenderer, InlineFlowState, inline_node_width,
    };
    use std::collections::HashMap;

    #[test]
    fn inline_node_width_uses_text_content_and_skips_block_nodes() {
        let inline = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "a".to_string(),
            attributes: vec![(
                "style".to_string(),
                "display:inline-block;padding:6px".to_string(),
            )],
            children: vec![HtmlDocumentNode::Text("Open link".to_string())],
        };
        let block = HtmlDocumentNode::Element {
            node_id: 2,
            tag: "div".to_string(),
            attributes: Vec::new(),
            children: Vec::new(),
        };

        assert!(inline_node_width(&inline, &CssStyle::browser_default(), 300.0).is_some());
        assert_eq!(
            inline_node_width(&block, &CssStyle::browser_default(), 300.0),
            None
        );
    }

    #[test]
    fn inline_flow_wraps_then_flushes_before_a_block() {
        let viewport = HtmlBrowserViewport {
            width: 100,
            height: 100,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let style = CssStyle::browser_default();
        let node = HtmlDocumentNode::Text("wrapped".to_string());
        let mut inline = InlineFlowState::new(0.0, 0.0, 20.0);
        inline.has_items = true;
        inline.cursor_x = 15.0;
        inline.bottom = 20.0;

        renderer.render_inline_node(&node, 10.0, &style, DetailsContext::NONE, &mut inline);
        assert_eq!(inline.y, 20.0);
        renderer.render_block_node(&node, &style, DetailsContext::NONE, &mut inline);
        assert!(!inline.has_items);
    }

    #[test]
    fn inline_node_width_respects_explicit_content_width() {
        let node = HtmlDocumentNode::Element {
            node_id: 3,
            tag: "a".to_string(),
            attributes: vec![(
                "style".to_string(),
                "display:inline-block;width:40px;padding:6px".to_string(),
            )],
            children: Vec::new(),
        };

        assert_eq!(
            inline_node_width(&node, &CssStyle::browser_default(), 300.0),
            Some(52.0)
        );
    }

    #[test]
    fn leading_inline_whitespace_becomes_a_horizontal_text_offset() {
        let style = CssStyle::browser_default();
        let (offset, visible) = super::visible_inline_line("  next", &style);

        assert!(offset > 0.0);
        assert_eq!(visible, "next");
        assert_eq!(super::visible_inline_line("next", &style), (0.0, "next"));
    }
}
