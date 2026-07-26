use super::super::html_browser::HtmlBrowserViewport;
use super::HtmlInteractiveSession;
use super::types::{ElementBox, HitTarget, LayoutResult};

impl HtmlInteractiveSession {
    pub(super) fn hit_target_at(&self, x: f32, y: f32) -> Option<&HitTarget> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale;
        let document_y = y / scale + self.scroll_y;
        self.hit_targets
            .iter()
            .rev()
            .find(|target| contains(*target, x, document_y))
    }

    pub(super) fn element_at(&self, x: f32, y: f32) -> Option<&ElementBox> {
        let scale = self.viewport.device_scale_factor;
        let x = x / scale;
        let document_y = y / scale + self.scroll_y;
        self.element_boxes
            .iter()
            .rev()
            .find(|element| contains(*element, x, document_y))
    }

    pub(super) fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport.logical_height()).max(0.0)
    }
}

pub(super) fn max_scroll_for(render: &LayoutResult, viewport: HtmlBrowserViewport) -> f32 {
    (render.content_height - viewport.logical_height()).max(0.0)
}

trait BoxGeometry {
    fn x(&self) -> f32;
    fn y(&self) -> f32;
    fn width(&self) -> f32;
    fn height(&self) -> f32;
}

impl BoxGeometry for HitTarget {
    fn x(&self) -> f32 {
        self.x
    }

    fn y(&self) -> f32 {
        self.y
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn height(&self) -> f32 {
        self.height
    }
}

impl BoxGeometry for ElementBox {
    fn x(&self) -> f32 {
        self.x
    }

    fn y(&self) -> f32 {
        self.y
    }

    fn width(&self) -> f32 {
        self.width
    }

    fn height(&self) -> f32 {
        self.height
    }
}

fn contains(target: &impl BoxGeometry, x: f32, y: f32) -> bool {
    x >= target.x()
        && x <= target.x() + target.width()
        && y >= target.y()
        && y <= target.y() + target.height()
}
