use super::super::html_document::HtmlDocumentNode;
use super::constants::{LIST_MARKER_WIDTH, MIN_LAYOUT_WIDTH, RULE_VERTICAL_INSET};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::svg::escape_xml;
use super::types::DetailsContext;

impl HtmlLayoutRenderer {
    pub(super) fn render_rule(&mut self, x: f32, y: f32, width: f32, style: &CssStyle) -> f32 {
        let line_y = y + style.margin_top + RULE_VERTICAL_INSET;
        let x = x + style.margin_left;
        let width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        self.svg.push_str(&format!(
            r#"<line x1="{x}" y1="{line_y}" x2="{}" y2="{line_y}" stroke="{}" stroke-width="1"/>"#,
            x + width,
            escape_xml(style.border.as_deref().unwrap_or("#c8cdd2"))
        ));
        line_y + RULE_VERTICAL_INSET + style.margin_bottom
    }

    pub(super) fn render_list(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        ordered: bool,
    ) -> f32 {
        let mut current = y + style.margin_top;
        let x = x + style.margin_left;
        let width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let mut index = 1usize;
        for child in children {
            if let Some(items) = list_item_children(child) {
                self.paint_list_marker(ordered, index, x, current, style);
                current = self.render_nodes(
                    items,
                    x + LIST_MARKER_WIDTH,
                    current,
                    width - LIST_MARKER_WIDTH,
                    style,
                    DetailsContext::NONE,
                );
                index += 1;
            }
        }
        current + style.margin_bottom
    }

    pub(super) fn render_list_item(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        self.paint_text_lines(
            &["•".to_string()],
            x,
            LIST_MARKER_WIDTH,
            y + style.font_size,
            style,
        );
        self.render_nodes(
            children,
            x + LIST_MARKER_WIDTH,
            y,
            width - LIST_MARKER_WIDTH,
            style,
            DetailsContext::NONE,
        )
    }

    fn paint_list_marker(&mut self, ordered: bool, index: usize, x: f32, y: f32, style: &CssStyle) {
        self.paint_text_lines(
            &[list_marker(ordered, index)],
            x,
            LIST_MARKER_WIDTH,
            y + style.font_size,
            style,
        );
    }
}

fn list_item_children(node: &HtmlDocumentNode) -> Option<&[HtmlDocumentNode]> {
    let HtmlDocumentNode::Element { tag, children, .. } = node else {
        return None;
    };
    (tag == "li").then_some(children)
}

fn list_marker(ordered: bool, index: usize) -> String {
    if ordered {
        format!("{index}.")
    } else {
        "•".to_string()
    }
}
