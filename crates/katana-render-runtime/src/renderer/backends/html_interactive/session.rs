use super::super::html_browser::{
    HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserPixelFormat, HtmlBrowserSource,
    HtmlBrowserViewport,
};
use super::super::html_runtime::{StaticHtmlRuntime, StaticHtmlRuntimeSession};
use super::super::html_subresources::HtmlSubresourcePolicy;
use super::document::seed_input_values;
use super::layout::HtmlLayoutRenderer;
use super::session_geometry::max_scroll_for;
use super::types::{ElementBox, HitTarget, LayoutResult};
use super::{HtmlBrowserError, runtime_failure};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use std::collections::{HashMap, HashSet};

/// In-process Rust/V8 HTML session. It owns the DOM, layout, hit-test and
/// raster frame so downstream crates only exchange frames and input events.
pub(in crate::renderer::backends) struct HtmlInteractiveSession {
    pub(super) source: HtmlBrowserSource,
    pub(super) viewport: HtmlBrowserViewport,
    pub(super) runtime: StaticHtmlRuntimeSession,
    pub(super) generation: u64,
    pub(super) latest_frame: Option<HtmlBrowserFrame>,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) element_boxes: Vec<ElementBox>,
    pub(super) hovered_nodes: HashSet<u64>,
    pub(super) input_values: HashMap<u64, String>,
    pub(super) pressed_target: Option<u64>,
    pub(super) focused_input: Option<u64>,
    pub(super) dirty_inputs: HashSet<u64>,
    pub(super) scroll_y: f32,
    pub(super) content_height: f32,
    pub(super) resize_anchor: Option<String>,
    pub(super) pending_navigation: Option<HtmlBrowserNavigationEvent>,
    pub(super) resource_policy: HtmlSubresourcePolicy,
}

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends) fn start(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, HtmlBrowserError> {
        source.validate()?;
        viewport.validate()?;
        let runtime = StaticHtmlRuntime
            .start_interactive(&source)
            .map_err(runtime_failure)?;
        let mut session = Self::new(source, viewport, runtime);
        session.render_frame()?;
        if session.source.origin.url().fragment().is_some() {
            let origin = session.source.origin.clone();
            session.apply_fragment_navigation(origin)?;
            session.render_frame()?;
        }
        Ok(session)
    }

    fn new(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        runtime: StaticHtmlRuntimeSession,
    ) -> Self {
        let resource_policy = HtmlSubresourcePolicy::from_source(&source);
        Self {
            source,
            viewport,
            runtime,
            generation: 0,
            latest_frame: None,
            hit_targets: Vec::new(),
            element_boxes: Vec::new(),
            hovered_nodes: HashSet::new(),
            input_values: HashMap::new(),
            pressed_target: None,
            focused_input: None,
            dirty_inputs: HashSet::new(),
            scroll_y: 0.0,
            content_height: 0.0,
            resize_anchor: None,
            pending_navigation: None,
            resource_policy,
        }
    }

    pub(in crate::renderer::backends) fn latest_frame(&self) -> Option<&HtmlBrowserFrame> {
        self.latest_frame.as_ref()
    }

    pub(in crate::renderer::backends) fn take_navigation(
        &mut self,
    ) -> Option<HtmlBrowserNavigationEvent> {
        self.pending_navigation.take()
    }

    pub(in crate::renderer::backends) fn refresh_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.render_frame()
    }

    pub(in crate::renderer::backends) fn resize(
        &mut self,
        viewport: HtmlBrowserViewport,
    ) -> Result<(), HtmlBrowserError> {
        viewport.validate()?;
        self.viewport = viewport;
        let layout = self.layout()?;
        let next_scroll = self
            .resize_anchor
            .as_ref()
            .and_then(|anchor| layout.anchor_positions.get(anchor))
            .copied()
            .unwrap_or(self.scroll_y);
        self.scroll_y = next_scroll.clamp(0.0, max_scroll_for(&layout, viewport));
        self.render_frame()
    }

    pub(super) fn render_frame(&mut self) -> Result<(), HtmlBrowserError> {
        let render = self.layout()?;
        self.update_scroll(&render);
        let frame = self.rasterize(&render.svg)?;
        self.store_frame(render, frame)
    }

    pub(super) fn layout(&mut self) -> Result<LayoutResult, HtmlBrowserError> {
        let nodes = self
            .runtime
            .interactive_nodes_at_width_with_hover(
                self.viewport.logical_width(),
                &self.hovered_nodes,
            )
            .map_err(runtime_failure)?;
        let clickable_nodes = self
            .runtime
            .event_target_ids("click")
            .map_err(runtime_failure)?;
        seed_input_values(&nodes, &mut self.input_values);
        HtmlLayoutRenderer::render_with_clickable_nodes(
            &nodes,
            self.viewport,
            self.scroll_y,
            &self.input_values,
            self.focused_input,
            &clickable_nodes,
        )
        .map_err(runtime_failure)
    }

    fn update_scroll(&mut self, render: &LayoutResult) {
        self.scroll_y = self
            .scroll_y
            .min((render.content_height - self.viewport.logical_height()).max(0.0));
    }

    fn rasterize(&self, svg: &str) -> Result<Vec<u8>, HtmlBrowserError> {
        let raster = SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
            .map_err(|error| runtime_failure(error.to_string()))?;
        self.validate_raster_dimensions(raster.width, raster.height)?;
        Ok(raster.rgba)
    }

    pub(super) fn validate_raster_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), HtmlBrowserError> {
        if width == self.viewport.width && height == self.viewport.height {
            return Ok(());
        }
        Err(runtime_failure(format!(
            "interactive raster dimensions are {width}x{height}, expected {}x{}",
            self.viewport.width, self.viewport.height
        )))
    }

    fn store_frame(
        &mut self,
        render: LayoutResult,
        pixels: Vec<u8>,
    ) -> Result<(), HtmlBrowserError> {
        self.generation += 1;
        self.latest_frame = Some(
            HtmlBrowserFrame::new(
                self.generation,
                self.source.origin.clone(),
                self.viewport,
                HtmlBrowserPixelFormat::Rgba8,
                pixels,
            )
            .map_err(|error| runtime_failure(error.to_string()))?
            .with_layout_metrics(self.scroll_y, render.content_height),
        );
        self.hit_targets = render.hit_targets;
        self.element_boxes = render.element_boxes;
        self.content_height = render.content_height;
        Ok(())
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
