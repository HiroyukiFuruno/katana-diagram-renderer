use super::super::html_document::HtmlDocumentNode;
use super::constants::{MIN_LAYOUT_WIDTH, TEXT_CHARACTER_WIDTH_FACTOR};
use super::document::node_text;
use super::layout_grid_measure::grid_track_widths;
use super::style::{CssLength, CssStyle};
use taffy::geometry::Size;
use taffy::prelude::{Display, Style};
use taffy::style_helpers::length;

pub(super) fn leaf_style(style: CssStyle, width: f32, height: f32) -> Style {
    Style {
        display: Display::Block,
        size: Size {
            width: length(width),
            height: length(height),
        },
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        ..Style::default()
    }
}

pub(super) fn item_style(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    width: f32,
    count: usize,
) -> CssStyle {
    match node {
        HtmlDocumentNode::Element { attributes, .. } => {
            CssStyle::from_attributes(attributes, inherited)
        }
        HtmlDocumentNode::Text(_) => {
            let mut style = inherited.clone();
            style.width = Some(CssLength::Px(width / count.max(1) as f32));
            style
        }
    }
}

pub(super) fn measured_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    width: f32,
    parent: &CssStyle,
) -> Vec<f32> {
    if parent.display != Display::Grid {
        return non_grid_measured_widths(nodes, styles, width);
    }

    let track_widths = grid_track_widths(nodes, styles, width, parent);
    styles
        .iter()
        .enumerate()
        .map(|(index, style)| {
            style
                .explicit_width(width)
                .unwrap_or(track_widths[index % track_widths.len()])
                .min(width)
                .max(MIN_LAYOUT_WIDTH)
        })
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
            style.explicit_width(width).unwrap_or_else(|| {
                intrinsic_text_width(node, style).min(width / nodes.len().max(1) as f32)
            })
        })
        .map(|measured| measured.min(width).max(MIN_LAYOUT_WIDTH))
        .collect()
}

pub(super) fn is_layout_item(node: &HtmlDocumentNode) -> bool {
    !matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty())
}

pub(super) fn intrinsic_text_width(node: &HtmlDocumentNode, style: &CssStyle) -> f32 {
    text_width(node_text(std::slice::from_ref(node)).chars().count(), style)
}

pub(super) fn min_content_text_width(node: &HtmlDocumentNode, style: &CssStyle) -> f32 {
    let text = node_text(std::slice::from_ref(node));
    let characters = text
        .split_whitespace()
        .map(str::chars)
        .map(Iterator::count)
        .max()
        .map_or(0, |characters| characters);
    text_width(characters, style)
}

fn text_width(characters: usize, style: &CssStyle) -> f32 {
    characters as f32 * style.font_size * TEXT_CHARACTER_WIDTH_FACTOR
        + style.padding_left
        + style.padding_right
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

        assert!(!is_layout_item(&whitespace));
        assert!(is_layout_item(&text));
        assert!(intrinsic_text_width(&text, &CssStyle::browser_default()) > 30.0);
    }

    #[test]
    fn text_and_grid_items_receive_deterministic_measurements() {
        let text = HtmlDocumentNode::Text("item".to_string());
        let inherited = CssStyle::browser_default();
        let text_style = item_style(&text, &inherited, 100.0, 2);
        assert_eq!(text_style.width, Some(CssLength::Px(50.0)));

        let mut grid = CssStyle::browser_default();
        grid.display = Display::Grid;
        grid.grid_template_columns = vec![CssGridTrack::Fraction(1.0); 2];
        grid.gap = 10.0;
        let items = [&text, &text];
        let styles = [inherited.clone(), inherited.clone()];
        assert_eq!(measured_widths(&items, &styles, 100.0, &grid), [45.0, 45.0]);
        assert!(measured_widths(&items, &styles, 100.0, &inherited)[0] > 30.0);
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
