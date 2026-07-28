pub(super) struct InlineFlowState {
    pub(super) x: f32,
    pub(super) width: f32,
    pub(super) cursor_x: f32,
    pub(super) y: f32,
    pub(super) bottom: f32,
    pub(super) has_items: bool,
}

impl InlineFlowState {
    pub(super) fn new(x: f32, y: f32, width: f32) -> Self {
        Self {
            x,
            width,
            cursor_x: x,
            y,
            bottom: y,
            has_items: false,
        }
    }

    pub(super) fn bottom(&self) -> f32 {
        if self.has_items { self.bottom } else { self.y }
    }
}
