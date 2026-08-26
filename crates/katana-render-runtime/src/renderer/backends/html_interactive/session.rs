use super::super::html_browser::{
    HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserSource, HtmlBrowserViewport,
};
use super::super::html_debug_trace::HtmlDebugTrace;
use super::super::html_runtime::{StaticHtmlRuntime, StaticHtmlRuntimeSession};
use super::super::html_subresources::HtmlSubresourcePolicy;
use super::session_geometry::max_scroll_for;
use super::types::{ElementBox, HitTarget};
use super::{HtmlBrowserError, runtime_failure};
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
    pub(super) trace: HtmlDebugTrace,
}

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends) fn start(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, HtmlBrowserError> {
        source.validate()?;
        viewport.validate()?;
        let trace = HtmlDebugTrace::from_env();
        let runtime = StaticHtmlRuntime
            .start_interactive_traced(&source, &trace)
            .map_err(runtime_failure)?;
        let mut session = Self::new(source, viewport, runtime, trace);
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
        trace: HtmlDebugTrace,
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
            trace,
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
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
