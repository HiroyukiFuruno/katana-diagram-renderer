use super::super::super::html_document::HtmlDocumentNode;
use super::super::layout_flow_measure::is_layout_item;
use super::super::layout_grid_track::taffy_grid_track;
use super::super::style::CssStyle;
use taffy::geometry::Size;
use taffy::prelude::{AvailableSpace, Display, Style, TaffyTree};
use taffy::style_helpers::{auto, length};

const GRID_ROW_Y_EPSILON: f32 = 0.01;

pub(super) fn flow_stretch_heights(
    tree: &TaffyTree<()>,
    parent: &CssStyle,
    measurements: &[(taffy::tree::NodeId, CssStyle, f32)],
) -> Result<Option<Vec<f32>>, String> {
    if !stretches_children(parent) {
        return Ok(None);
    }
    if parent.display == Display::Flex && is_flex_row(parent) {
        let height = measurements
            .iter()
            .map(|(_, _, height)| *height)
            .fold(0.0, f32::max);
        return Ok(Some(vec![height; measurements.len()]));
    }
    if parent.display != Display::Grid {
        return Ok(None);
    }
    grid_row_heights(tree, measurements).map(Some)
}

fn grid_row_heights(
    tree: &TaffyTree<()>,
    measurements: &[(taffy::tree::NodeId, CssStyle, f32)],
) -> Result<Vec<f32>, String> {
    let mut heights = Vec::with_capacity(measurements.len());
    for (node, _, _) in measurements {
        let row_y = tree.layout(*node).map_err(layout_error)?.location.y;
        heights.push(grid_row_height(tree, measurements, row_y)?);
    }
    Ok(heights)
}

fn grid_row_height(
    tree: &TaffyTree<()>,
    measurements: &[(taffy::tree::NodeId, CssStyle, f32)],
    row_y: f32,
) -> Result<f32, String> {
    let mut row_height = 0.0_f32;
    for (candidate, _, measured_height) in measurements {
        let candidate_y = tree.layout(*candidate).map_err(layout_error)?.location.y;
        if (candidate_y - row_y).abs() <= GRID_ROW_Y_EPSILON {
            row_height = row_height.max(*measured_height);
        }
    }
    Ok(row_height)
}

fn stretches_children(parent: &CssStyle) -> bool {
    matches!(
        parent.align_items,
        None | Some(taffy::style::AlignItems::STRETCH)
    )
}

fn is_flex_row(parent: &CssStyle) -> bool {
    matches!(
        parent.flex_direction,
        taffy::style::FlexDirection::Row | taffy::style::FlexDirection::RowReverse
    )
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
