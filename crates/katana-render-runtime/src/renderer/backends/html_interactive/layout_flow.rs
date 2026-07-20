use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::layout::HtmlLayoutRenderer;
use super::layout_flow_measure::{is_layout_item, item_style, leaf_style, measured_width};
use super::style::CssStyle;
use super::types::DetailsContext;
use taffy::geometry::Size;
use taffy::prelude::{AvailableSpace, Display, Style, TaffyTree};
use taffy::style_helpers::{auto, fr, length};

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
            .filter(|node| is_layout_item(node))
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
        let mut nodes = Vec::with_capacity(items.len());
        for item in items {
            let item_style = item_style(item, style, width, items.len());
            let measure_width = measured_width(item, &item_style, width, style, items.len());
            let height = self.measure_node_height(item, measure_width, style, details)?;
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
            let item_bottom = self.render_node(
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
        layout.grid_template_columns = vec![fr(1.0_f32); style.grid_columns.max(1)];
    }
    layout
}

fn layout_error(error: impl ToString) -> String {
    format!("CSS flow layout failed: {}", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, DetailsContext, Display, HtmlDocumentNode};
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
}
