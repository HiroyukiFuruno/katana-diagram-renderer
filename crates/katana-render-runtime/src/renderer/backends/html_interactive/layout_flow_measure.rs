use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::layout_grid_measure::grid_track_widths;
use super::style::{CssLength, CssStyle};
use taffy::geometry::Size;
use taffy::prelude::{Display, Style};
use taffy::style_helpers::{auto, length, percent};

#[path = "layout_intrinsic.rs"]
mod intrinsic;

use intrinsic::intrinsic_layout_width;
pub(super) use intrinsic::{intrinsic_text_width, is_layout_item, min_content_text_width};

pub(super) fn leaf_style(style: CssStyle, width: f32, height: f32, available_width: f32) -> Style {
    Style {
        display: Display::Block,
        size: Size {
            width: length(width),
            height: length(height),
        },
        min_size: Size {
            width: style
                .minimum_outer_width(available_width)
                .map_or_else(auto, length),
            height: length(leaf_minimum_height(&style, height)),
        },
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis: match style.flex_basis {
            Some(CssLength::Px(value)) => length(value),
            Some(CssLength::Percent(value)) => percent(value),
            None => auto(),
        },
        ..Style::default()
    }
}

pub(super) fn assign_leaf_height(style: &mut Style, css_style: &CssStyle, height: f32) {
    style.size.height = length(height);
    style.min_size.height = length(leaf_minimum_height(css_style, height));
}

fn leaf_minimum_height(style: &CssStyle, measured_height: f32) -> f32 {
    if style.automatic_min_height {
        measured_height
    } else {
        style.minimum_outer_height() + style.margin_top + style.margin_bottom
    }
    .max(0.0)
}

pub(super) fn item_style(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    _width: f32,
    _count: usize,
) -> CssStyle {
    match node {
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } => CssStyle::from_element(tag, attributes, inherited),
        HtmlDocumentNode::Text(_) => inherited.clone(),
    }
}

pub(super) fn measured_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    width: f32,
    parent: &CssStyle,
) -> Vec<f32> {
    if stretches_flex_columns(parent) {
        return stretched_column_widths(styles, width);
    }
    if parent.display != Display::Grid {
        return non_grid_measured_widths(nodes, styles, width);
    }

    let track_widths = grid_track_widths(nodes, styles, width, parent);
    styles
        .iter()
        .enumerate()
        .map(|(index, style)| {
            let measured = style
                .explicit_width(width)
                .unwrap_or(track_widths[index % track_widths.len()])
                .min(width);
            enforce_min_width(style, measured, width).max(MIN_LAYOUT_WIDTH)
        })
        .collect()
}

fn stretches_flex_columns(style: &CssStyle) -> bool {
    style.display == Display::Flex
        && matches!(
            style.flex_direction,
            taffy::style::FlexDirection::Column | taffy::style::FlexDirection::ColumnReverse
        )
        && matches!(
            style.align_items,
            None | Some(taffy::style::AlignItems::STRETCH)
        )
}

fn stretched_column_widths(styles: &[CssStyle], width: f32) -> Vec<f32> {
    styles
        .iter()
        .map(|style| enforce_min_width(style, style.explicit_width(width).unwrap_or(width), width))
        .map(|measured| measured.min(width).max(MIN_LAYOUT_WIDTH))
        .collect()
}

fn non_grid_measured_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    width: f32,
) -> Vec<f32> {
    nodes
        .iter()
        .zip(styles)
        .map(|(node, style)| {
            let measured = style
                .explicit_width(width)
                .unwrap_or_else(|| intrinsic_layout_width(node, style, width));
            enforce_min_width(style, measured, width)
        })
        .map(|measured| measured.min(width).max(MIN_LAYOUT_WIDTH))
        .collect()
}

fn enforce_min_width(style: &CssStyle, measured: f32, available: f32) -> f32 {
    style
        .minimum_outer_width(available)
        .map_or(measured, |minimum| measured.max(minimum))
}

#[cfg(test)]
mod tests {
    use super::super::style::{CssGridTrack, CssGridTrackBreadth};
    use super::{CssLength, CssStyle, Display, intrinsic_text_width, is_layout_item};
    use super::{item_style, measured_widths};
    use crate::renderer::backends::html_document::HtmlDocumentNode;

    #[test]
    fn flow_helpers_ignore_formatting_whitespace_and_measure_text() {
        let whitespace = HtmlDocumentNode::Text("  \n ".to_string());
        let text = HtmlDocumentNode::Text("abcd".to_string());
        let japanese = HtmlDocumentNode::Text("案件概要".to_string());

        assert!(!is_layout_item(&whitespace));
        assert!(is_layout_item(&text));
        assert!(intrinsic_text_width(&text, &CssStyle::browser_default()) > 30.0);
        assert!(
            intrinsic_text_width(&japanese, &CssStyle::browser_default())
                > intrinsic_text_width(&text, &CssStyle::browser_default())
        );
    }

    #[test]
    fn text_and_grid_items_receive_deterministic_measurements() {
        let text = HtmlDocumentNode::Text("item".to_string());
        let inherited = CssStyle::browser_default();
        let text_style = item_style(&text, &inherited, 100.0, 2);
        assert_eq!(text_style.width, None);

        let mut grid = CssStyle::browser_default();
        grid.display = Display::Grid;
        grid.grid_template_columns = vec![CssGridTrack::Fraction(1.0); 2];
        grid.gap = 10.0;
        let items = [&text, &text];
        let styles = [inherited.clone(), inherited.clone()];
        assert_eq!(measured_widths(&items, &styles, 100.0, &grid), [45.0, 45.0]);
        assert!(measured_widths(&items, &styles, 100.0, &inherited)[0] > 30.0);

        let mut column = inherited.clone();
        column.display = Display::Flex;
        column.flex_direction = taffy::style::FlexDirection::Column;
        assert_eq!(
            measured_widths(&items, &styles, 100.0, &column),
            [100.0, 100.0]
        );
    }

    #[test]
    fn leaf_style_forwards_typed_flex_basis_to_taffy() {
        let mut style = CssStyle::browser_default();
        style.flex_basis = Some(CssLength::Percent(0.0));
        let leaf = super::leaf_style(style, 80.0, 20.0, 200.0);
        assert_eq!(leaf.flex_basis, taffy::style_helpers::percent(0.0_f32));

        let mut pixels = CssStyle::browser_default();
        pixels.flex_basis = Some(CssLength::Px(24.0));
        let leaf = super::leaf_style(pixels, 80.0, 20.0, 200.0);
        assert_eq!(leaf.flex_basis, taffy::style_helpers::length(24.0_f32));

        let leaf = super::leaf_style(CssStyle::browser_default(), 80.0, 20.0, 200.0);
        assert_eq!(leaf.flex_basis, taffy::style_helpers::auto());
    }

    #[test]
    fn explicit_stretch_alignment_expands_flex_column_items() {
        let text = HtmlDocumentNode::Text("item".to_string());
        let styles = [CssStyle::browser_default()];
        let mut column = CssStyle::browser_default();
        column.display = Display::Flex;
        column.flex_direction = taffy::style::FlexDirection::Column;
        column.align_items = Some(taffy::style::AlignItems::STRETCH);

        assert_eq!(measured_widths(&[&text], &styles, 120.0, &column), [120.0]);
    }

    #[test]
    fn reassigned_leaf_height_updates_size_and_automatic_minimum() {
        let style = CssStyle::browser_default();
        let mut leaf = super::leaf_style(style.clone(), 80.0, 20.0, 200.0);
        super::assign_leaf_height(&mut leaf, &style, 64.0);
        assert_eq!(leaf.size.height, taffy::style_helpers::length(64.0_f32));
        assert_eq!(leaf.min_size.height, taffy::style_helpers::length(64.0_f32));

        let mut explicit = style;
        explicit.automatic_min_height = false;
        explicit.min_height = 12.0;
        super::assign_leaf_height(&mut leaf, &explicit, 64.0);
        assert_eq!(leaf.min_size.height, taffy::style_helpers::length(12.0_f32));
    }

    #[test]
    fn grid_measurements_preserve_fixed_intrinsic_and_fraction_tracks() {
        let first = HtmlDocumentNode::Text("Menu".to_string());
        let second = HtmlDocumentNode::Text("Content".to_string());
        let style = CssStyle::browser_default();
        let items = [&first, &second];
        let styles = [style.clone(), style.clone()];

        let mut fixed = style.clone();
        fixed.display = Display::Grid;
        fixed.grid_template_columns =
            vec![CssGridTrack::Length(100.0), CssGridTrack::Fraction(1.0)];
        fixed.gap = 12.0;
        assert_eq!(
            measured_widths(&items, &styles, 280.0, &fixed),
            [100.0, 168.0]
        );

        fixed.grid_template_columns = vec![CssGridTrack::MaxContent, CssGridTrack::Fraction(1.0)];
        let measured = measured_widths(&items, &styles, 280.0, &fixed);
        assert!(measured[0] < measured[1]);
    }

    #[test]
    fn grid_measurements_respect_minmax_minimums_and_fraction_growth() {
        let first = HtmlDocumentNode::Text("First".to_string());
        let second = HtmlDocumentNode::Text("Second".to_string());
        let style = CssStyle::browser_default();
        let items = [&first, &second];
        let styles = [style.clone(), style.clone()];
        let mut grid = style;
        grid.display = Display::Grid;
        grid.gap = 10.0;
        grid.grid_template_columns = vec![
            CssGridTrack::MinMax {
                min: CssGridTrackBreadth::Length(80.0),
                max: CssGridTrackBreadth::Fraction(1.0),
            },
            CssGridTrack::MinMax {
                min: CssGridTrackBreadth::Length(120.0),
                max: CssGridTrackBreadth::Fraction(1.0),
            },
        ];

        assert_eq!(
            measured_widths(&items, &styles, 310.0, &grid),
            [150.0, 150.0]
        );
    }

    #[test]
    fn grid_measurements_cover_percent_min_content_auto_and_empty_tracks() {
        let first = HtmlDocumentNode::Text("long word".to_string());
        let second = HtmlDocumentNode::Text("content".to_string());
        let style = CssStyle::browser_default();
        let items = [&first, &second];
        let styles = [style.clone(), style.clone()];
        let mut grid = style;
        grid.display = Display::Grid;
        grid.grid_template_columns = vec![CssGridTrack::Percent(0.25), CssGridTrack::MinContent];
        let intrinsic = measured_widths(&items, &styles, 200.0, &grid);
        assert_eq!(intrinsic[0], 50.0);
        assert!(intrinsic[1] > 40.0);

        grid.grid_template_columns = vec![CssGridTrack::Length(50.0), CssGridTrack::Auto];
        assert_eq!(
            measured_widths(&items, &styles, 200.0, &grid),
            [50.0, 150.0]
        );
        grid.grid_template_columns = vec![CssGridTrack::Length(50.0)];
        assert_eq!(
            measured_widths(&items[..1], &styles[..1], 200.0, &grid),
            [50.0]
        );
        grid.grid_template_columns.clear();
        assert_eq!(
            measured_widths(&items[..1], &styles[..1], 200.0, &grid),
            [200.0]
        );
    }
}
