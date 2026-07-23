use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::document::node_text;
use super::super::style::{CssStyle, CssTextAlign};
use super::super::text_metrics::text_width;

pub(super) fn inline_run_start_x(
    nodes: &[HtmlDocumentNode],
    x: f32,
    available: f32,
    style: &CssStyle,
) -> f32 {
    let width = nodes
        .iter()
        .filter(|node| !matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty()))
        .map_while(|node| inline_node_width(node, style, available))
        .sum::<f32>()
        .min(available);
    match style.text_align {
        CssTextAlign::Start => x,
        CssTextAlign::Center => x + (available - width).max(0.0) / 2.0,
        CssTextAlign::End => x + (available - width).max(0.0),
    }
}

pub(super) fn inline_node_width(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    available: f32,
) -> Option<f32> {
    match node {
        HtmlDocumentNode::Text(text) => Some(inline_text_width(text, inherited).min(available)),
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => inline_element_width(tag, attributes, children, inherited, available),
    }
}

fn inline_element_width(
    tag: &str,
    attributes: &[(String, String)],
    children: &[HtmlDocumentNode],
    inherited: &CssStyle,
    available: f32,
) -> Option<f32> {
    let style = CssStyle::from_element(tag, attributes, inherited);
    if !style.inline_block || style.display == taffy::style::Display::None {
        return None;
    }
    let available_box = (available - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
    let box_width = inline_content_box_width(children, &style, available_box);
    Some(
        (box_width + style.margin_left + style.margin_right)
            .min(available)
            .max(MIN_LAYOUT_WIDTH),
    )
}

fn inline_content_box_width(
    children: &[HtmlDocumentNode],
    style: &CssStyle,
    available: f32,
) -> f32 {
    if style.width.is_some() || style.max_width.is_some() {
        style.box_width(available)
    } else {
        let content = inline_text_width(&node_text(children), style);
        style.outer_width(content)
    }
}

pub(super) fn inline_text_width(text: &str, style: &CssStyle) -> f32 {
    text_width(text, style)
}

pub(super) fn visible_inline_line<'a>(line: &'a str, style: &CssStyle) -> (f32, &'a str) {
    let visible = line.trim_start_matches(char::is_whitespace);
    let leading = &line[..line.len() - visible.len()];
    (text_width(leading, style), visible)
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, CssTextAlign, HtmlDocumentNode, inline_run_start_x};

    #[test]
    fn end_aligned_inline_run_starts_after_available_space() {
        let nodes = [HtmlDocumentNode::Text("short".to_string())];
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::End;

        assert!(inline_run_start_x(&nodes, 10.0, 200.0, &style) > 100.0);
    }
}
