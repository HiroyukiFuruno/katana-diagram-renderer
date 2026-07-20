use super::super::html_browser::HtmlBrowserViewport;
use super::HtmlInteractiveSession;
use super::types::{HitTarget, LayoutResult};

impl HtmlInteractiveSession {
    pub(super) fn hit_target_at(&self, x: f32, y: f32) -> Option<&HitTarget> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale;
        let document_y = y / scale + self.scroll_y;
        self.hit_targets
            .iter()
            .rev()
            .find(|target| contains(target, x, document_y))
    }

    pub(super) fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport.logical_height()).max(0.0)
    }
}

pub(super) fn max_scroll_for(render: &LayoutResult, viewport: HtmlBrowserViewport) -> f32 {
    (render.content_height - viewport.logical_height()).max(0.0)
}

fn contains(target: &HitTarget, x: f32, y: f32) -> bool {
    x >= target.x && x <= target.x + target.width && y >= target.y && y <= target.y + target.height
}
