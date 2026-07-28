use super::super::document::{attribute, input_initial_value};
use super::super::style::CssStyle;

pub(super) fn is_checked(attributes: &[(String, String)]) -> bool {
    attribute(attributes, "checked").is_some()
}

pub(super) fn input_value(
    style: &mut super::super::layout::HtmlLayoutRenderer,
    node_id: u64,
    attributes: &[(String, String)],
) -> String {
    style
        .input_values
        .entry(node_id)
        .or_insert_with(|| input_initial_value(attributes))
        .clone()
}

pub(super) fn input_display_value<'a>(
    attributes: &'a [(String, String)],
    value: &'a str,
    style: &CssStyle,
) -> (&'a str, CssStyle) {
    if !value.is_empty() {
        return (value, style.clone());
    }
    let Some(placeholder) = attribute(attributes, "placeholder") else {
        return (value, style.clone());
    };
    let mut placeholder_style = style.clone();
    placeholder_style.color = "#9ca3af".to_string();
    placeholder_style.italic = true;
    (placeholder, placeholder_style)
}
