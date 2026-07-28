use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::LAYOUT_FLOAT_EPSILON;
use super::super::document::wrap_text_with_initial_width;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::{CssStyle, CssTextAlign};
use super::super::types::DetailsContext;
use super::InlineMeasurement;
use super::floats::InlineFloat;
use super::state::InlineFlowState;
use super::{advance_inline_text, inline_flow_style};

impl HtmlLayoutRenderer {
    pub(crate) fn render_nodes(
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
        if self.render_inline_float(node, inherited, details, inline) {
            return;
        }
        if !inline.has_items {
            inline.cursor_x =
                InlineMeasurement::run_start_x(&nodes[index..], inline.x, inline.width, inherited);
        }
        if let HtmlDocumentNode::Text(text) = node {
            self.render_inline_text(text, inherited, inline);
        } else if let Some(style) = inline_flow_style(node, inherited, &self.clickable_nodes) {
            self.render_inline_flow_children(node, &style, details, inline);
        } else if let Some(width) = InlineMeasurement::node_width(node, inherited, inline.width) {
            self.render_inline_node(node, width, inherited, details, inline);
        } else {
            self.render_block_node(node, inherited, details, inline);
        }
    }

    fn render_inline_float(
        &mut self,
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &InlineFlowState,
    ) -> bool {
        let Some((side, float_width)) = InlineFloat::node_geometry(node, inherited, inline.width)
        else {
            return false;
        };
        self.render_floated_node(node, side, float_width, inherited, details, inline);
        true
    }

    pub(super) fn render_inline_flow_children(
        &mut self,
        node: &HtmlDocumentNode,
        style: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        let HtmlDocumentNode::Element { children, .. } = node else {
            return;
        };
        for index in 0..children.len() {
            self.render_inline_or_block_node(children, index, style, details, inline);
        }
    }

    pub(super) fn render_inline_node(
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
        let remaining_width =
            (inline.x + inline.width - initial_x).max(super::super::constants::MIN_LAYOUT_WIDTH);
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
            let (leading_width, visible_line) = InlineMeasurement::visible_line(line, style);
            let line_x = line_x + leading_width;
            let line_y = initial_y + index as f32 * style.line_height;
            let visible_line = visible_line.to_string();
            self.paint_text_lines(
                std::slice::from_ref(&visible_line),
                line_x,
                (inline.x + inline.width - line_x).max(super::super::constants::MIN_LAYOUT_WIDTH),
                line_y + style.font_size,
                &paint_style,
            );
        }
    }

    pub(super) fn render_block_node(
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
