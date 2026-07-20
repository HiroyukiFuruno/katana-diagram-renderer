use super::super::html_document::HtmlDocumentNode;
use super::constants::{MIN_LAYOUT_WIDTH, TEXT_CHARACTER_WIDTH_FACTOR};
use super::document::node_text;
use super::style::{CssLength, CssStyle};
use taffy::geometry::Size;
use taffy::prelude::{Display, Style};
use taffy::style_helpers::length;

pub(super) fn leaf_style(style: CssStyle, width: f32, height: f32) -> Style {
    Style {
        display: Display::Block,
        size: Size {
            width: length(width),
            height: length(height),
        },
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        ..Style::default()
    }
}

pub(super) fn item_style(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    width: f32,
    count: usize,
) -> CssStyle {
    match node {
        HtmlDocumentNode::Element { attributes, .. } => {
            CssStyle::from_attributes(attributes, inherited)
        }
        HtmlDocumentNode::Text(_) => {
            let mut style = inherited.clone();
            style.width = Some(CssLength::Px(width / count.max(1) as f32));
            style
        }
    }
}

pub(super) fn measured_width(
    node: &HtmlDocumentNode,
    style: &CssStyle,
    width: f32,
    parent: &CssStyle,
    count: usize,
) -> f32 {
    if let Some(preferred) = style.explicit_width(width) {
        return preferred.min(width).max(MIN_LAYOUT_WIDTH);
    }
    if parent.display == Display::Grid {
        let gaps = parent.gap * parent.grid_columns.saturating_sub(1) as f32;
        return ((width - gaps) / parent.grid_columns.max(1) as f32).max(MIN_LAYOUT_WIDTH);
    }
    intrinsic_text_width(node, style)
        .min(width / count.max(1) as f32)
        .max(MIN_LAYOUT_WIDTH)
}

pub(super) fn is_layout_item(node: &HtmlDocumentNode) -> bool {
    !matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty())
}

fn intrinsic_text_width(node: &HtmlDocumentNode, style: &CssStyle) -> f32 {
    node_text(std::slice::from_ref(node)).chars().count() as f32
        * style.font_size
        * TEXT_CHARACTER_WIDTH_FACTOR
        + style.padding_left
        + style.padding_right
}

#[cfg(test)]
mod tests {
    use super::{CssLength, CssStyle, Display, intrinsic_text_width, is_layout_item};
    use super::{item_style, measured_width};
    use crate::renderer::backends::html_document::HtmlDocumentNode;

    #[test]
    fn flow_helpers_ignore_formatting_whitespace_and_measure_text() {
        let whitespace = HtmlDocumentNode::Text("  \n ".to_string());
        let text = HtmlDocumentNode::Text("abcd".to_string());

        assert!(!is_layout_item(&whitespace));
        assert!(is_layout_item(&text));
        assert!(intrinsic_text_width(&text, &CssStyle::browser_default()) > 30.0);
    }

    #[test]
    fn text_and_grid_items_receive_deterministic_measurements() {
        let text = HtmlDocumentNode::Text("item".to_string());
        let inherited = CssStyle::browser_default();
        let text_style = item_style(&text, &inherited, 100.0, 2);
        assert_eq!(text_style.width, Some(CssLength::Px(50.0)));

        let mut grid = CssStyle::browser_default();
        grid.display = Display::Grid;
        grid.grid_columns = 2;
        grid.gap = 10.0;
        assert_eq!(measured_width(&text, &inherited, 100.0, &grid, 2), 45.0);
        assert!(measured_width(&text, &inherited, 100.0, &inherited, 2) > 30.0);
    }
}
