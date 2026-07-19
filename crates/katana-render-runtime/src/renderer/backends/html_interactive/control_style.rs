use super::super::html_document::HtmlDocumentNode;
use super::constants::{BUTTON_TEXT_HORIZONTAL_PADDING, BUTTON_TEXT_WIDTH_FACTOR, CONTROL_HEIGHT};
use super::document::{attribute, node_text};
use super::style::CssStyle;

pub(super) fn button_width(children: &[HtmlDocumentNode], width: f32, style: &CssStyle) -> f32 {
    style.width.unwrap_or_else(|| {
        (node_text(children).chars().count() as f32 * style.font_size * BUTTON_TEXT_WIDTH_FACTOR
            + BUTTON_TEXT_HORIZONTAL_PADDING)
            .min(width)
    })
}

pub(super) fn button_style(style: &CssStyle) -> CssStyle {
    let mut style = style.clone();
    if !style.explicit_background {
        style.background = Some("#e9ecef".to_string());
    }
    style.border = Some(
        style
            .border
            .clone()
            .unwrap_or_else(|| "#6c757d".to_string()),
    );
    style
}

pub(super) fn input_style(style: &CssStyle, focused: bool) -> CssStyle {
    let mut style = style.clone();
    style.background = Some(
        style
            .background
            .clone()
            .unwrap_or_else(|| "#ffffff".to_string()),
    );
    style.border = Some(if focused { "#0969da" } else { "#8c959f" }.to_string());
    style
}

pub(super) fn visible_details_children(
    attributes: &[(String, String)],
    children: &[HtmlDocumentNode],
) -> Vec<HtmlDocumentNode> {
    let open = attribute(attributes, "open").is_some();
    children
        .iter()
        .filter(|child| is_summary(child) || open)
        .cloned()
        .collect()
}

pub(super) fn summary_height(style: &CssStyle) -> f32 {
    (style.height.unwrap_or(CONTROL_HEIGHT) + style.padding * 2.0).max(style.min_height)
}

fn is_summary(node: &&HtmlDocumentNode) -> bool {
    matches!(node, HtmlDocumentNode::Element { tag, .. } if tag == "summary")
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, button_style};

    #[test]
    fn button_style_keeps_an_explicit_background() {
        let style = CssStyle {
            background: Some("#123456".to_string()),
            explicit_background: true,
            ..CssStyle::default()
        };

        assert_eq!(button_style(&style).background.as_deref(), Some("#123456"));
    }
}
