use super::super::html_document::HtmlDocumentNode;
use super::constants::BUTTON_TEXT_LEFT_PADDING;
use super::document::node_text;
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::text_metrics::text_width;
use super::types::{ControlLayout, ElementRenderContext, HitTarget, HitTargetKind};
use taffy::style::{AlignItems, Display, FlexDirection, JustifyContent};

#[derive(Clone, Copy)]
struct ControlTextLayout {
    x: f32,
    width: f32,
    y: f32,
    height: f32,
}

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
        self.paint_button_text(element.children, layout);
        self.push_target(
            element.node_id,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            HitTargetKind::Click,
        );
    }

    fn paint_button_text(&mut self, children: &[HtmlDocumentNode], layout: ControlLayout<'_>) {
        let text = node_text(children);
        let text_layout = button_text_layout(&text, layout);
        self.paint_control_text(
            &text,
            text_layout.x,
            text_layout.width,
            text_layout.y,
            text_layout.height,
            layout.style,
        );
    }

    pub(super) fn paint_control_text(
        &mut self,
        text: &str,
        x: f32,
        width: f32,
        start: f32,
        height: f32,
        style: &CssStyle,
    ) {
        let baseline = start + (height + style.font_size) / 2.0 - 2.0;
        self.paint_text_lines(&[text.to_string()], x, width, baseline, style);
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

fn button_text_layout(text: &str, layout: ControlLayout<'_>) -> ControlTextLayout {
    let mut text_layout = ControlTextLayout {
        x: layout.x + BUTTON_TEXT_LEFT_PADDING,
        width: (layout.width - BUTTON_TEXT_LEFT_PADDING * 2.0).max(0.0),
        y: layout.y,
        height: layout.height,
    };
    let Some((horizontal_centered, vertical_centered)) = flex_center_axes(layout.style) else {
        return text_layout;
    };

    if horizontal_centered {
        let content_x = layout.x + layout.style.border_left_width() + layout.style.padding_left;
        let content_width = layout.style.content_width(layout.width);
        let measured_width = text_width(text, layout.style).min(content_width);
        text_layout.x = content_x + (content_width - measured_width) / 2.0;
        text_layout.width = measured_width;
    }
    if vertical_centered {
        text_layout.y = layout.y + layout.style.border_top_width() + layout.style.padding_top;
        text_layout.height = layout.style.content_height(layout.height);
    }
    text_layout
}

fn flex_center_axes(style: &CssStyle) -> Option<(bool, bool)> {
    (style.display == Display::Flex).then(|| {
        let row_axis = matches!(
            style.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        if row_axis {
            (
                style.justify_content == Some(JustifyContent::CENTER),
                style.align_items == Some(AlignItems::CENTER),
            )
        } else {
            (
                style.align_items == Some(AlignItems::CENTER),
                style.justify_content == Some(JustifyContent::CENTER),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{ControlLayout, button_text_layout};
    use crate::renderer::backends::html_interactive::{style::CssStyle, text_metrics::text_width};
    use taffy::style::{AlignItems, Display, FlexDirection, JustifyContent};

    #[test]
    fn column_flex_centers_button_text_on_cross_and_main_axes() {
        let mut style = CssStyle::browser_default();
        style.display = Display::Flex;
        style.flex_direction = FlexDirection::Column;
        style.align_items = Some(AlignItems::CENTER);
        style.justify_content = Some(JustifyContent::CENTER);
        style.border_width = 1.0;
        let layout = ControlLayout {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 40.0,
            style: &style,
        };

        let text_layout = button_text_layout("‹", layout);
        let measured_width = text_width("‹", &style);

        assert!((text_layout.x - (20.0 - measured_width / 2.0)).abs() <= f32::EPSILON);
        assert!((text_layout.width - measured_width).abs() <= f32::EPSILON);
        assert!((text_layout.y - 1.0).abs() <= f32::EPSILON);
        assert!((text_layout.height - 38.0).abs() <= f32::EPSILON);
    }
}
