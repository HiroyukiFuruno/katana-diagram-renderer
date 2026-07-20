use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::layout::HtmlLayoutRenderer;
use super::layout_flow_measure::{is_layout_item, item_style, leaf_style, measured_widths};
use super::style::{CssGridTrack, CssStyle};
use super::types::DetailsContext;
use taffy::geometry::Size;
use taffy::prelude::{AvailableSpace, Display, Style, TaffyTree};
use taffy::style_helpers::{auto, fr, length, max_content, min_content, percent};

impl HtmlLayoutRenderer {
    pub(super) fn render_flow_children(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        match style.display {
            Display::Flex | Display::Grid => {
                self.render_taffy_flow(children, x, y, width, style, details)
            }
            Display::Block => Ok(self.render_nodes(children, x, y, width, style, details)),
            Display::None => Ok(y),
        }
    }

    fn render_taffy_flow(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let items = children
            .iter()
            .filter(|node| is_visible_layout_item(node, style))
            .collect::<Vec<_>>();
        if items.is_empty() {
            return Ok(y);
        }
        let mut tree = TaffyTree::<()>::new();
        let nodes = self.build_flow_nodes(&mut tree, &items, width, style, details)?;
        compute_flow_layout(&mut tree, &nodes, style, width)?;
        self.paint_flow_items(&tree, &nodes, &items, (x, y), style, details)
    }

    fn build_flow_nodes(
        &mut self,
        tree: &mut TaffyTree<()>,
        items: &[&HtmlDocumentNode],
        width: f32,
        style: &CssStyle,
        details: DetailsContext,
    ) -> Result<Vec<taffy::tree::NodeId>, String> {
        let item_styles = items
            .iter()
            .map(|item| item_style(item, style, width, items.len()))
            .collect::<Vec<_>>();
        let widths = measured_widths(items, &item_styles, width, style);
        let mut nodes = Vec::with_capacity(items.len());
        for ((item, item_style), measure_width) in items.iter().zip(item_styles).zip(widths) {
            let height = self.measure_flow_node_height(item, measure_width, style, details)?;
            let node = tree
                .new_leaf(leaf_style(item_style, measure_width, height))
                .map_err(layout_error)?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    fn paint_flow_items(
        &mut self,
        tree: &TaffyTree<()>,
        nodes: &[taffy::tree::NodeId],
        items: &[&HtmlDocumentNode],
        origin: (f32, f32),
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> Result<f32, String> {
        let (x, y) = origin;
        let mut bottom = y;
        for (node, item) in nodes.iter().zip(items) {
            let layout = tree.layout(*node).map_err(layout_error)?;
            let item_bottom = self.render_flow_node(
                item,
                x + layout.location.x,
                y + layout.location.y,
                layout.size.width.max(MIN_LAYOUT_WIDTH),
                inherited,
                details,
            );
            bottom = bottom.max(item_bottom);
        }
        Ok(bottom)
    }
}

fn is_visible_layout_item(node: &HtmlDocumentNode, inherited: &CssStyle) -> bool {
    if !is_layout_item(node) {
        return false;
    }

    match node {
        HtmlDocumentNode::Text(_) => true,
        HtmlDocumentNode::Element { attributes, .. } => {
            let style = CssStyle::from_attributes(attributes, inherited);
            style.display != Display::None
        }
    }
}

fn compute_flow_layout(
    tree: &mut TaffyTree<()>,
    nodes: &[taffy::tree::NodeId],
    style: &CssStyle,
    width: f32,
) -> Result<(), String> {
    let root = tree
        .new_with_children(flow_style(style, width), nodes)
        .map_err(layout_error)?;
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::MaxContent,
        },
    )
    .map_err(layout_error)
}

fn flow_style(style: &CssStyle, width: f32) -> Style {
    let mut layout = Style {
        display: style.display,
        size: Size {
            width: length(width),
            height: auto(),
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

fn taffy_grid_track(track: CssGridTrack) -> taffy::style::TrackSizingFunction {
    match track {
        CssGridTrack::Length(value) => length(value),
        CssGridTrack::Percent(value) => percent(value),
        CssGridTrack::Fraction(value) => fr(value),
        CssGridTrack::Auto => auto(),
        CssGridTrack::MinContent => min_content(),
        CssGridTrack::MaxContent => max_content(),
    }
}

fn layout_error(error: impl ToString) -> String {
    format!("CSS flow layout failed: {}", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::taffy_grid_track;
    use super::{CssGridTrack, CssStyle, DetailsContext, Display, HtmlDocumentNode};
    use super::{HtmlLayoutRenderer, layout_error};
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    #[test]
    fn none_and_empty_flow_paths_preserve_the_current_position() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let mut style = CssStyle::browser_default();
        style.display = Display::None;
        assert_eq!(
            renderer.render_flow_children(&[], 0.0, 7.0, 100.0, &style, DetailsContext::NONE),
            Ok(7.0)
        );

        style.display = Display::Flex;
        let whitespace = [HtmlDocumentNode::Text(" \n ".to_string())];
        assert_eq!(
            renderer.render_flow_children(
                &whitespace,
                0.0,
                9.0,
                100.0,
                &style,
                DetailsContext::NONE,
            ),
            Ok(9.0)
        );
        assert_eq!(layout_error("boom"), "CSS flow layout failed: boom");
    }

    #[test]
    fn grid_track_conversion_preserves_supported_css_sizing_kinds() {
        let percent = taffy_grid_track(CssGridTrack::Percent(0.25));
        assert_eq!(
            percent
                .max_sizing_function()
                .definite_value(Some(200.0), |_, _| 0.0),
            Some(50.0)
        );
        assert!(
            taffy_grid_track(CssGridTrack::Auto)
                .max_sizing_function()
                .is_auto()
        );
        assert!(
            taffy_grid_track(CssGridTrack::MinContent)
                .max_sizing_function()
                .is_min_content()
        );
        assert!(
            taffy_grid_track(CssGridTrack::MaxContent)
                .max_sizing_function()
                .is_max_content()
        );
    }
}
