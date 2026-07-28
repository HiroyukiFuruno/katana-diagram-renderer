use super::super::super::html_document::HtmlDocumentNode;
use super::super::document::node_text;
use super::super::layout_media::intrinsic_image_width;
use super::super::style::CssStyle;
use super::super::text_metrics::text_width as measured_text_width;
use taffy::prelude::Display;

pub(in crate::renderer::backends::html_interactive) fn is_layout_item(
    node: &HtmlDocumentNode,
) -> bool {
    !matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty())
}

pub(in crate::renderer::backends::html_interactive) fn intrinsic_text_width(
    node: &HtmlDocumentNode,
    style: &CssStyle,
) -> f32 {
    intrinsic_styled_node_width(node, style, style.viewport_width)
}

pub(in crate::renderer::backends::html_interactive) fn intrinsic_layout_width(
    node: &HtmlDocumentNode,
    style: &CssStyle,
    available: f32,
) -> f32 {
    if let Some(width) = intrinsic_image_width(node, available, style) {
        return width;
    }
    let HtmlDocumentNode::Element { children, .. } = node else {
        return measured_text_width(&node_text(std::slice::from_ref(node)), style);
    };
    if style.display != Display::Flex {
        return style
            .intrinsic_outer_width(intrinsic_inline_children_width(children, style, available));
    }
    let widths = children
        .iter()
        .filter_map(|child| intrinsic_flex_child_width(child, style, available))
        .collect::<Vec<_>>();
    let content_width = if matches!(
        style.flex_direction,
        taffy::style::FlexDirection::Column | taffy::style::FlexDirection::ColumnReverse
    ) {
        widths.into_iter().reduce(f32::max).unwrap_or(0.0)
    } else {
        let gaps = widths.len().saturating_sub(1) as f32 * style.gap;
        widths.into_iter().sum::<f32>() + gaps
    };
    style.intrinsic_outer_width(content_width)
}

fn intrinsic_inline_children_width(
    children: &[HtmlDocumentNode],
    inherited: &CssStyle,
    available: f32,
) -> f32 {
    let mut maximum = 0.0_f32;
    let mut inline_run = 0.0_f32;
    for child in children {
        let style = child_style(child, inherited);
        if style.display == Display::None {
            continue;
        }
        let width = intrinsic_styled_node_width(child, &style, available)
            + style.margin_left
            + style.margin_right;
        if matches!(child, HtmlDocumentNode::Text(_)) || style.inline_block {
            inline_run += width;
        } else {
            maximum = maximum.max(inline_run).max(width);
            inline_run = 0.0;
        }
    }
    maximum.max(inline_run)
}

fn intrinsic_styled_node_width(node: &HtmlDocumentNode, style: &CssStyle, available: f32) -> f32 {
    if let Some(width) = style.explicit_width(available) {
        return width;
    }
    if let Some(width) = intrinsic_image_width(node, available, style) {
        return width;
    }
    match node {
        HtmlDocumentNode::Text(text) => measured_text_width(text, style),
        HtmlDocumentNode::Element { children, .. } => {
            style.intrinsic_outer_width(intrinsic_inline_children_width(children, style, available))
        }
    }
}

fn child_style(node: &HtmlDocumentNode, inherited: &CssStyle) -> CssStyle {
    match node {
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } => CssStyle::from_element(tag, attributes, inherited),
        HtmlDocumentNode::Text(_) => inherited.inherited_text_style(),
    }
}

fn intrinsic_flex_child_width(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    available: f32,
) -> Option<f32> {
    if !is_layout_item(node) {
        return None;
    }
    let style = child_style(node, inherited);
    if style.display == Display::None {
        return None;
    }
    let width = style
        .explicit_width(available)
        .unwrap_or_else(|| intrinsic_layout_width(node, &style, available));
    Some(width + style.margin_left + style.margin_right)
}

pub(in crate::renderer::backends::html_interactive) fn min_content_text_width(
    node: &HtmlDocumentNode,
    style: &CssStyle,
) -> f32 {
    match node {
        HtmlDocumentNode::Text(text) => text
            .split_whitespace()
            .map(|word| measured_text_width(word, style))
            .reduce(f32::max)
            .unwrap_or(0.0),
        HtmlDocumentNode::Element { children, .. } => {
            let content = children
                .iter()
                .map(|child| {
                    let child_style = child_style(child, style);
                    min_content_text_width(child, &child_style)
                })
                .reduce(f32::max)
                .unwrap_or(0.0);
            style.intrinsic_outer_width(content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CssStyle, Display, HtmlDocumentNode, intrinsic_layout_width, intrinsic_styled_node_width,
        min_content_text_width,
    };

    fn element(tag: &str, style: &str, children: Vec<HtmlDocumentNode>) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id: 1,
            tag: tag.to_string(),
            attributes: vec![("style".to_string(), style.to_string())],
            children,
        }
    }

    #[test]
    fn flex_column_intrinsic_width_uses_widest_child() {
        let node = element(
            "div",
            "",
            vec![
                HtmlDocumentNode::Text("short".to_string()),
                HtmlDocumentNode::Text("considerably wider".to_string()),
            ],
        );
        let mut style = CssStyle::browser_default();
        style.display = Display::Flex;
        style.flex_direction = taffy::style::FlexDirection::Column;

        assert!(intrinsic_layout_width(&node, &style, 400.0) > 100.0);
    }

    #[test]
    fn inline_intrinsic_width_skips_hidden_and_flushes_block_children() {
        let node = element(
            "div",
            "",
            vec![
                element(
                    "span",
                    "display: none",
                    vec![HtmlDocumentNode::Text("hidden".into())],
                ),
                HtmlDocumentNode::Text("inline".to_string()),
                element(
                    "div",
                    "",
                    vec![HtmlDocumentNode::Text("block content".into())],
                ),
            ],
        );

        assert!(intrinsic_layout_width(&node, &CssStyle::browser_default(), 400.0) > 50.0);
    }

    #[test]
    fn styled_image_uses_png_intrinsic_width() {
        let source = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB";
        let node = HtmlDocumentNode::Element {
            node_id: 1,
            tag: "img".to_string(),
            attributes: vec![("src".to_string(), source.to_string())],
            children: Vec::new(),
        };

        assert_eq!(
            intrinsic_styled_node_width(&node, &CssStyle::browser_default(), 100.0),
            1.0
        );
    }

    #[test]
    fn element_min_content_width_recurses_through_children() {
        let node = element(
            "span",
            "padding: 2px",
            vec![HtmlDocumentNode::Text("longest-word".to_string())],
        );

        assert!(min_content_text_width(&node, &CssStyle::browser_default()) > 80.0);
    }
}
