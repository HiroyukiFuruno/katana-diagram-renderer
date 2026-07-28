use super::super::super::html_browser::HtmlBrowserViewport;
use super::super::types::DetailsContext;
use super::{
    CssFloat, CssStyle, HtmlDocumentNode, HtmlLayoutRenderer, InlineFloat, InlineFlowState,
    InlineMeasurement,
};
use std::collections::HashMap;

#[test]
fn inline_node_width_uses_text_content_and_skips_block_nodes() {
    let inline = HtmlDocumentNode::Element {
        node_id: 1,
        tag: "a".to_string(),
        attributes: vec![(
            "style".to_string(),
            "display:inline-block;padding:6px".to_string(),
        )],
        children: vec![HtmlDocumentNode::Text("Open link".to_string())],
    };
    let block = HtmlDocumentNode::Element {
        node_id: 2,
        tag: "div".to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    };

    assert!(InlineMeasurement::node_width(&inline, &CssStyle::browser_default(), 300.0).is_some());
    assert_eq!(
        InlineMeasurement::node_width(&block, &CssStyle::browser_default(), 300.0),
        None
    );
}

#[test]
fn inline_flow_wraps_then_flushes_before_a_block() {
    let viewport = HtmlBrowserViewport {
        width: 100,
        height: 100,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let style = CssStyle::browser_default();
    let node = HtmlDocumentNode::Text("wrapped".to_string());
    let mut inline = InlineFlowState::new(0.0, 0.0, 20.0);
    inline.has_items = true;
    inline.cursor_x = 15.0;
    inline.bottom = 20.0;

    renderer.render_inline_flow_children(&node, &style, DetailsContext::NONE, &mut inline);
    renderer.render_inline_node(&node, 10.0, &style, DetailsContext::NONE, &mut inline);
    assert_eq!(inline.y, 20.0);
    renderer.render_block_node(&node, &style, DetailsContext::NONE, &mut inline);
    assert!(!inline.has_items);
}

#[test]
fn inline_node_width_respects_explicit_content_width() {
    let node = HtmlDocumentNode::Element {
        node_id: 3,
        tag: "a".to_string(),
        attributes: vec![(
            "style".to_string(),
            "display:inline-block;width:40px;padding:6px".to_string(),
        )],
        children: Vec::new(),
    };

    assert_eq!(
        InlineMeasurement::node_width(&node, &CssStyle::browser_default(), 300.0),
        Some(52.0)
    );
}

#[test]
fn leading_inline_whitespace_becomes_a_horizontal_text_offset() {
    let style = CssStyle::browser_default();
    let (offset, visible) = InlineMeasurement::visible_line("  next", &style);

    assert!(offset > 0.0);
    assert_eq!(visible, "next");
    assert_eq!(
        InlineMeasurement::visible_line("next", &style),
        (0.0, "next")
    );
}

#[test]
fn floated_nodes_shrink_to_content_and_hidden_or_normal_nodes_stay_in_flow() -> Result<(), String> {
    let floated = HtmlDocumentNode::Element {
        node_id: 4,
        tag: "span".to_string(),
        attributes: vec![("style".to_string(), "float:right;margin:0 5px".to_string())],
        children: vec![HtmlDocumentNode::Text("count".to_string())],
    };
    let hidden = HtmlDocumentNode::Element {
        node_id: 5,
        tag: "span".to_string(),
        attributes: vec![("style".to_string(), "float:left;display:none".to_string())],
        children: Vec::new(),
    };
    let normal = HtmlDocumentNode::Text("normal".to_string());

    let (side, width) = InlineFloat::node_geometry(&floated, &CssStyle::browser_default(), 200.0)
        .ok_or("float geometry was missing")?;
    assert_eq!(side, CssFloat::Right);
    assert!(width > 10.0);
    assert!(InlineFloat::node_geometry(&hidden, &CssStyle::browser_default(), 200.0).is_none());
    assert!(InlineFloat::node_geometry(&normal, &CssStyle::browser_default(), 200.0).is_none());
    Ok(())
}
