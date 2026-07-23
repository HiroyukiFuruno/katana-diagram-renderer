use super::layout::{ContainingBlock, HtmlLayoutRenderer};
use super::style::{CssPosition, CssStyle};

fn horizontal_position(containing: ContainingBlock, style: &CssStyle) -> (f32, f32) {
    let left = style.inset_left;
    let right = style.inset_right;
    let width = match (left, right) {
        (Some(left), Some(right)) => (containing.width - left - right).max(0.0),
        _ => style.box_width(containing.width).min(containing.width),
    };
    let x = left.map_or_else(
        || {
            right.map_or(containing.x, |right| {
                containing.x + containing.width - right - width
            })
        },
        |left| containing.x + left,
    );
    (x, width)
}

fn vertical_position(containing: ContainingBlock, style: &CssStyle) -> (f32, Option<f32>) {
    let top = style.inset_top;
    let bottom = style.inset_bottom;
    let height = match (top, bottom) {
        (Some(top), Some(bottom)) => Some((containing.height - top - bottom).max(0.0)),
        _ => style.height.map(|height| style.outer_height(height)),
    };
    let y = top.map_or_else(
        || {
            bottom.zip(height).map_or(containing.y, |(bottom, height)| {
                containing.y + containing.height - bottom - height
            })
        },
        |top| containing.y + top,
    );
    (y, height)
}

impl HtmlLayoutRenderer {
    pub(super) fn positioned_geometry(&self, style: &mut CssStyle) -> (f32, f32, f32) {
        let containing = self.positioning_containing_block(style.position);
        let (x, width) = horizontal_position(containing, style);
        let (y, height) = vertical_position(containing, style);
        if let Some(height) = height {
            style.assign_outer_height(height);
        }
        style.width = None;
        style.max_width = None;
        (x, y, width)
    }

    fn positioning_containing_block(&self, position: CssPosition) -> ContainingBlock {
        if position == CssPosition::Fixed {
            return ContainingBlock {
                x: 0.0,
                y: self.scroll_y,
                width: self.viewport_width,
                height: self.viewport_height,
            };
        }
        self.containing_blocks
            .last()
            .copied()
            .unwrap_or(ContainingBlock {
                x: 0.0,
                y: 0.0,
                width: self.viewport_width,
                height: self.viewport_height,
            })
    }

    pub(super) fn push_containing_block(&mut self, block: ContainingBlock) {
        self.containing_blocks.push(block);
    }

    pub(super) fn pop_containing_block(&mut self) {
        self.containing_blocks.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainingBlock, CssStyle, horizontal_position};

    fn containing_block() -> ContainingBlock {
        ContainingBlock {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            height: 100.0,
        }
    }

    #[test]
    fn horizontal_position_defaults_to_containing_block_start() {
        assert_eq!(
            horizontal_position(containing_block(), &CssStyle::browser_default()),
            (10.0, 200.0)
        );
    }

    #[test]
    fn horizontal_position_honors_right_inset_with_explicit_width() {
        let mut style = CssStyle::browser_default();
        style.width = Some(super::super::style::CssLength::Px(50.0));
        style.inset_right = Some(15.0);

        assert_eq!(
            horizontal_position(containing_block(), &style),
            (145.0, 50.0)
        );
    }
}
