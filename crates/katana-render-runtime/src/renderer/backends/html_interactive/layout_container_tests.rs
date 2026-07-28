use super::helpers::{
    accept_flow_result, container_height, horizontal_box_geometry, inline_container_width,
};
use super::{CssStyle, HtmlDocumentNode};

#[test]
fn flow_errors_are_recorded_at_the_container_start() {
    let mut layout_error = None;

    assert_eq!(
        accept_flow_result(&mut layout_error, Err("taffy failed".to_string()), 12.0),
        12.0
    );
    assert_eq!(layout_error, Some("taffy failed".to_string()));
}

#[test]
fn inline_container_shrinks_to_text_content() {
    let children = [HtmlDocumentNode::Text("Compact label".to_string())];
    let mut style = CssStyle::browser_default();
    style.inline_block = true;

    assert!(inline_container_width(&children, 300.0, &style) < 120.0);
    assert_eq!(inline_container_width(&children, 40.0, &style), 40.0);
}

#[test]
fn inline_container_width_includes_margins_exactly_once() {
    let children = [HtmlDocumentNode::Text("Label".to_string())];
    let mut style = CssStyle::browser_default();
    style.inline_block = true;
    let without_margins = inline_container_width(&children, 300.0, &style);

    style.margin_left = 10.0;
    style.margin_right = 5.0;
    assert_eq!(
        inline_container_width(&children, 300.0, &style),
        without_margins + 15.0
    );
}

#[test]
fn container_height_is_clamped_by_max_height() {
    let mut style = CssStyle::browser_default();
    style.max_height = Some(20.0);

    assert_eq!(container_height(100.0, 0.0, &style), 20.0);
}

#[test]
fn horizontal_auto_margins_center_a_constrained_box() {
    let mut style = CssStyle::browser_default();
    style.max_width = Some(super::super::style::CssLength::Px(100.0));
    style.margin_left_auto = true;
    style.margin_right_auto = true;

    assert_eq!(horizontal_box_geometry(0.0, 320.0, &style), (110.0, 100.0));
}
