use super::{CssBoxSizing, CssOverflow, CssStyle};

impl CssStyle {
    pub(in super::super) fn explicit_width(&self, available: f32) -> Option<f32> {
        self.width.map(|width| {
            let resolved = self.outer_width(width.resolve(available));
            self.max_width.map_or(resolved, |maximum| {
                resolved.min(self.outer_width(maximum.resolve(available)))
            })
        })
    }

    pub(in super::super) fn box_width(&self, available: f32) -> f32 {
        let width = self.explicit_width(available).unwrap_or(available);
        self.max_width.map_or(width, |maximum| {
            width.min(self.outer_width(maximum.resolve(available)))
        })
    }

    pub(in super::super) fn content_width(&self, outer_width: f32) -> f32 {
        (outer_width - self.horizontal_non_content()).max(0.0)
    }

    pub(in super::super) fn outer_height(&self, css_height: f32) -> f32 {
        match self.box_sizing {
            CssBoxSizing::ContentBox => css_height + self.vertical_non_content(),
            CssBoxSizing::BorderBox => css_height,
        }
    }

    pub(in super::super) fn minimum_outer_height(&self) -> f32 {
        self.outer_height(self.min_height)
    }

    pub(in super::super) fn clips_overflow(&self) -> bool {
        self.overflow == CssOverflow::Clip
    }

    pub(in super::super) fn consume_assigned_flow_width(&mut self) {
        if self.width.take().is_some() {
            self.max_width = None;
        }
    }

    pub(in super::super) fn outer_width(&self, css_width: f32) -> f32 {
        match self.box_sizing {
            CssBoxSizing::ContentBox => css_width + self.horizontal_non_content(),
            CssBoxSizing::BorderBox => css_width,
        }
    }

    fn horizontal_non_content(&self) -> f32 {
        self.padding_left + self.padding_right + self.border_width * 2.0
    }

    fn vertical_non_content(&self) -> f32 {
        self.padding_top + self.padding_bottom + self.border_width * 2.0
    }
}
