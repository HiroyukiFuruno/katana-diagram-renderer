use super::super::html_document::HtmlDocumentNode;
use super::constants::{MIN_LAYOUT_WIDTH, TEXT_CHARACTER_WIDTH_FACTOR};
use super::document::node_text;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::DetailsContext;

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
        for node in nodes {
            if matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty()) {
                continue;
            }
            if let Some(inline_width) = inline_node_width(node, inherited, width) {
                self.render_inline_node(node, inline_width, inherited, details, &mut inline);
            } else {
                self.render_block_node(node, inherited, details, &mut inline);
            }
        }
        inline.bottom()
    }

    fn render_inline_node(
        &mut self,
        node: &HtmlDocumentNode,
        inline_width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        if inline.has_items && inline.cursor_x + inline_width > inline.x + inline.width {
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

fn inline_node_width(node: &HtmlDocumentNode, inherited: &CssStyle, available: f32) -> Option<f32> {
    let HtmlDocumentNode::Element {
        attributes,
        children,
        ..
    } = node
    else {
        return None;
    };
    let style = CssStyle::from_attributes(attributes, inherited);
    if !style.inline_block || style.display == taffy::style::Display::None {
        return None;
    }
    let available_box = (available - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let box_width = if style.width.is_some() || style.max_width.is_some() {
        style.box_width(available_box)
    } else {
        let content = node_text(children).chars().count() as f32
            * style.font_size
            * TEXT_CHARACTER_WIDTH_FACTOR;
        style.outer_width(content)
    };
    Some(
        (box_width + style.margin_left + style.margin_right)
            .min(available)
            .max(MIN_LAYOUT_WIDTH),
    )
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
}
