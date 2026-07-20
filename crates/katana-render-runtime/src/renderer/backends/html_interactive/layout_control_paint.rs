use super::constants::BUTTON_TEXT_LEFT_PADDING;
use super::document::node_text;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::{ControlLayout, ElementRenderContext, HitTarget, HitTargetKind};

impl HtmlLayoutRenderer {
    pub(super) fn paint_button(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: ControlLayout<'_>,
    ) {
        self.paint_box(
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            layout.style,
        );
        self.paint_control_text(
            &node_text(element.children),
            layout.x + BUTTON_TEXT_LEFT_PADDING,
            layout.y,
            layout.height,
            layout.style,
        );
        self.push_target(
            element.node_id,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            HitTargetKind::Click,
        );
    }

    pub(super) fn paint_control_text(
        &mut self,
        text: &str,
        x: f32,
        start: f32,
        height: f32,
        style: &CssStyle,
    ) {
        let baseline = start + (height + style.font_size) / 2.0 - 2.0;
        self.paint_text_lines(&[text.to_string()], x, baseline, style);
    }

    pub(super) fn push_target(
        &mut self,
        node_id: u64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        kind: HitTargetKind,
    ) {
        self.hit_targets.push(HitTarget {
            node_id,
            x,
            y,
            width,
            height,
            kind,
        });
    }
}
