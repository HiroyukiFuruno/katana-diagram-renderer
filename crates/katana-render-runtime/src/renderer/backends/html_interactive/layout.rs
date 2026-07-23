use super::super::html_browser::HtmlBrowserViewport;
use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::style::CssStyle;
use super::svg::svg_header;
use super::types::{DetailsContext, HitTarget, LayoutResult};
use std::collections::HashMap;

pub(super) struct HtmlLayoutRenderer {
    pub(super) scroll_y: f32,
    pub(super) svg: String,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) anchor_positions: HashMap<String, f32>,
    pub(super) input_values: HashMap<u64, String>,
    pub(super) focused_input: Option<u64>,
    pub(super) layout_error: Option<String>,
    pub(super) viewport_height: f32,
    pub(super) viewport_width: f32,
    pub(super) next_clip_id: u64,
    pub(super) next_gradient_id: u64,
    pub(super) containing_blocks: Vec<ContainingBlock>,
    pub(super) clickable_nodes: std::collections::HashSet<u64>,
    pub(super) document_paint_start: usize,
    pub(super) deferred_paint: Vec<DeferredPaint>,
    pub(super) next_paint_order: u64,
}

pub(super) struct DeferredPaint {
    pub(super) z_index: i32,
    pub(super) order: u64,
    pub(super) svg: String,
}

#[derive(Clone, Copy)]
pub(super) struct ContainingBlock {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

impl HtmlLayoutRenderer {
    #[cfg(test)]
    pub(super) fn render(
        nodes: &[HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Result<LayoutResult, String> {
        Self::render_with_clickable_nodes(
            nodes,
            viewport,
            scroll_y,
            input_values,
            focused_input,
            &std::collections::HashSet::new(),
        )
    }

    pub(super) fn render_with_clickable_nodes(
        nodes: &[HtmlDocumentNode],
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
        clickable_nodes: &std::collections::HashSet<u64>,
    ) -> Result<LayoutResult, String> {
        let mut renderer = Self::new_with_clickable_nodes(
            viewport,
            scroll_y,
            input_values,
            focused_input,
            clickable_nodes,
        );
        let width = viewport.logical_width().max(MIN_LAYOUT_WIDTH);
        let root_style = CssStyle::browser_default_for_viewport(
            viewport.logical_width(),
            viewport.logical_height(),
        );
        let bottom =
            renderer.render_nodes(nodes, 0.0, 0.0, width, &root_style, DetailsContext::NONE);
        renderer.into_layout_result(bottom)
    }

    fn into_layout_result(mut self, content_height: f32) -> Result<LayoutResult, String> {
        self.ensure_layout_succeeded()?;
        self.finish_deferred_paint();
        self.svg.push_str("</svg>");
        Ok(LayoutResult {
            svg: self.svg,
            hit_targets: self.hit_targets,
            anchor_positions: self.anchor_positions,
            content_height,
        })
    }

    pub(super) fn new(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
    ) -> Self {
        Self::new_with_clickable_nodes(
            viewport,
            scroll_y,
            input_values,
            focused_input,
            &std::collections::HashSet::new(),
        )
    }

    fn new_with_clickable_nodes(
        viewport: HtmlBrowserViewport,
        scroll_y: f32,
        input_values: &HashMap<u64, String>,
        focused_input: Option<u64>,
        clickable_nodes: &std::collections::HashSet<u64>,
    ) -> Self {
        let svg = svg_header(viewport);
        let document_paint_start = svg.len();
        Self {
            scroll_y,
            svg,
            hit_targets: Vec::new(),
            anchor_positions: HashMap::new(),
            input_values: input_values.clone(),
            focused_input,
            layout_error: None,
            viewport_height: viewport.logical_height(),
            viewport_width: viewport.logical_width(),
            next_clip_id: 0,
            next_gradient_id: 0,
            containing_blocks: Vec::new(),
            clickable_nodes: clickable_nodes.clone(),
            document_paint_start,
            deferred_paint: Vec::new(),
            next_paint_order: 0,
        }
    }

    pub(super) fn ensure_layout_succeeded(&mut self) -> Result<(), String> {
        self.layout_error.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlBrowserViewport, HtmlLayoutRenderer};
    use std::collections::HashMap;

    #[test]
    fn renderer_propagates_recorded_layout_errors() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        renderer.layout_error = Some("layout failed".to_string());

        assert_eq!(
            renderer.ensure_layout_succeeded(),
            Err("layout failed".to_string())
        );
    }
}
