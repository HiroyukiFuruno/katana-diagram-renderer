use super::super::super::html_document::HtmlDocumentNode;
use super::super::layout_flow_measure::is_layout_item;
use super::super::layout_grid_track::taffy_grid_track;
use super::super::style::CssStyle;
use taffy::geometry::Size;
use taffy::prelude::{AvailableSpace, Display, Style, TaffyTree};
use taffy::style_helpers::{auto, length};

pub(super) fn row_stretch_height(
    parent: &CssStyle,
    measurements: &[(taffy::tree::NodeId, CssStyle, f32)],
) -> Option<f32> {
    let is_row = matches!(
        parent.flex_direction,
        taffy::style::FlexDirection::Row | taffy::style::FlexDirection::RowReverse
    );
    let stretches = matches!(
        parent.align_items,
        None | Some(taffy::style::AlignItems::STRETCH)
    );
    (parent.display == Display::Flex && is_row && stretches).then(|| {
        measurements
            .iter()
            .map(|(_, _, height)| *height)
            .fold(0.0, f32::max)
    })
}

pub(super) fn is_visible_layout_item(node: &HtmlDocumentNode, inherited: &CssStyle) -> bool {
    if !is_layout_item(node) {
        return false;
    }
    match node {
        HtmlDocumentNode::Text(_) => true,
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } => CssStyle::from_element(tag, attributes, inherited).display != Display::None,
    }
}

pub(super) fn compute_flow_layout(
    tree: &mut TaffyTree<()>,
    nodes: &[taffy::tree::NodeId],
    style: &CssStyle,
    width: f32,
    height: Option<f32>,
) -> Result<taffy::tree::NodeId, String> {
    let root = tree
        .new_with_children(flow_style(style, width, height), nodes)
        .map_err(layout_error)?;
    compute_root_layout(tree, root, width, height)?;
    Ok(root)
}

pub(super) fn compute_root_layout(
    tree: &mut TaffyTree<()>,
    root: taffy::tree::NodeId,
    width: f32,
    height: Option<f32>,
) -> Result<(), String> {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(width),
            height: height.map_or(AvailableSpace::MaxContent, AvailableSpace::Definite),
        },
    )
    .map_err(layout_error)
}

fn flow_style(style: &CssStyle, width: f32, height: Option<f32>) -> Style {
    let mut layout = Style {
        display: style.display,
        size: Size {
            width: length(width),
            height: height.map_or_else(auto, length),
        },
        gap: Size {
            width: length(style.gap),
            height: length(style.gap),
        },
        flex_direction: style.flex_direction,
        flex_wrap: style.flex_wrap,
        align_items: style.align_items,
        justify_content: style.justify_content,
        ..Style::default()
    };
    if style.display == Display::Grid {
        layout.grid_template_columns = style
            .grid_template_columns
            .iter()
            .copied()
            .map(taffy_grid_track)
            .map(Into::into)
            .collect();
    }
    layout
}

pub(super) fn layout_error(error: impl ToString) -> String {
    format!("CSS flow layout failed: {}", error.to_string())
}
